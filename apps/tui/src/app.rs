use std::{io::Stdout, time::Duration};

use crossterm::event::{
    Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use futures_util::StreamExt;
use luna_protocol::{Bootstrap, Conversation, ConversationMessages, SendMessageResponse};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    api::LunaApi,
    input::{Composer, ComposerAction},
    realtime::{RealtimeUpdate, spawn_realtime},
    state::{ClientState, StateEffect},
    ui,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Transcript,
    Composer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusDirection {
    Left,
    Down,
    Up,
    Right,
}

const MOUSE_SCROLL_LINES: u16 = 3;

pub struct App {
    pub state: ClientState,
    pub focus: Focus,
    pub connection: ConnectionStatus,
    pub connection_detail: Option<String>,
    pub composer: Composer,
    pub list_index: usize,
    pub transcript_offset_from_bottom: u16,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub show_help: bool,
    pub confirm_interrupt: bool,
    pub pending_action: bool,
    deferred_message_load: Option<Uuid>,
    pub reset_required: bool,
    pub should_quit: bool,
    pub color: bool,
}

impl App {
    #[must_use]
    pub fn new(bootstrap: Bootstrap) -> Self {
        Self {
            state: ClientState::from_bootstrap(bootstrap),
            focus: Focus::List,
            connection: ConnectionStatus::Connecting,
            connection_detail: None,
            composer: Composer::default(),
            list_index: 0,
            transcript_offset_from_bottom: 0,
            error: None,
            notice: None,
            show_help: false,
            confirm_interrupt: false,
            pending_action: false,
            deferred_message_load: None,
            reset_required: false,
            should_quit: false,
            color: std::env::var_os("NO_COLOR").is_none(),
        }
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::List => Focus::Transcript,
            Focus::Transcript => Focus::Composer,
            Focus::Composer => Focus::List,
        };
    }

    fn move_focus(&mut self, direction: FocusDirection) {
        self.focus = match (self.focus, direction) {
            (Focus::List, FocusDirection::Right) => Focus::Transcript,
            (Focus::Transcript | Focus::Composer, FocusDirection::Left) => Focus::List,
            (Focus::Transcript, FocusDirection::Down) => Focus::Composer,
            (Focus::Composer, FocusDirection::Up) => Focus::Transcript,
            (focus, _) => focus,
        };
    }

    fn sync_list_index(&mut self) {
        if let Some(selected) = self.state.selected_conversation_id
            && let Some(index) = self
                .state
                .conversations
                .iter()
                .position(|conversation| conversation.id == selected)
        {
            self.list_index = index;
            return;
        }
        self.list_index = self
            .list_index
            .min(self.state.conversations.len().saturating_sub(1));
    }
}

pub async fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    api: LunaApi,
    bootstrap: Bootstrap,
) -> Result<(), AppError> {
    let mut app = App::new(bootstrap);
    let (realtime, mut realtime_updates) = spawn_realtime(api.clone(), app.state.cursor);
    let (actions_tx, mut actions_rx) = mpsc::channel(32);
    if let Some(conversation_id) = app.state.selected_conversation_id {
        spawn_load_messages(&mut app, &api, &actions_tx, conversation_id, None);
    }
    let mut terminal_events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let termination = termination_signal();
    tokio::pin!(termination);
    let mut regions = ui::UiRegions::default();

    while !app.should_quit {
        terminal.draw(|frame| regions = ui::render(frame, &app))?;
        tokio::select! {
            _ = tick.tick() => {}
            _ = &mut termination => app.should_quit = true,
            event = terminal_events.next() => {
                match event {
                    Some(Ok(event)) => {
                        handle_terminal_event(&mut app, event, &regions, &api, &actions_tx);
                    }
                    Some(Err(error)) => return Err(AppError::Terminal(error)),
                    None => app.should_quit = true,
                }
            }
            Some(update) = realtime_updates.recv() => {
                handle_realtime_update(&mut app, update, &api, &actions_tx);
                realtime.set_cursor(app.state.cursor);
            }
            Some(result) = actions_rx.recv() => {
                handle_action_result(&mut app, result, &api, &actions_tx);
                realtime.set_cursor(app.state.cursor);
            }
        }
    }

    realtime.shutdown().await;
    Ok(())
}

fn handle_terminal_event(
    app: &mut App,
    event: Event,
    regions: &ui::UiRegions,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
) {
    match event {
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            handle_key(app, key, api, actions);
        }
        Event::Paste(value) if app.focus == Focus::Composer && !app.pending_action => {
            app.composer.insert_paste(&value);
        }
        Event::Mouse(mouse) => handle_mouse(app, mouse, regions, api, actions),
        _ => {}
    }
}

fn handle_mouse(
    app: &mut App,
    mouse: MouseEvent,
    regions: &ui::UiRegions,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
) {
    if app.show_help || app.confirm_interrupt {
        return;
    }
    let over_list = regions.list_contains(mouse.column, mouse.row);
    let over_transcript = regions.transcript_contains(mouse.column, mouse.row);
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            app.notice = None;
            if let Some(index) = regions.conversation_at(mouse.column, mouse.row) {
                app.list_index = index;
                if select_conversation(app, index, api, actions) {
                    app.focus = Focus::List;
                }
            } else if over_list {
                app.focus = Focus::List;
            } else if over_transcript {
                app.focus = Focus::Transcript;
            } else if regions.composer_contains(mouse.column, mouse.row) {
                app.focus = Focus::Composer;
            }
        }
        MouseEventKind::ScrollUp if over_transcript => {
            app.focus = Focus::Transcript;
            app.transcript_offset_from_bottom = app
                .transcript_offset_from_bottom
                .saturating_add(MOUSE_SCROLL_LINES);
        }
        MouseEventKind::ScrollDown if over_transcript => {
            app.focus = Focus::Transcript;
            app.transcript_offset_from_bottom = app
                .transcript_offset_from_bottom
                .saturating_sub(MOUSE_SCROLL_LINES);
        }
        MouseEventKind::ScrollUp if over_list => {
            app.focus = Focus::List;
            app.list_index = app.list_index.saturating_sub(1);
        }
        MouseEventKind::ScrollDown if over_list => {
            app.focus = Focus::List;
            app.list_index =
                (app.list_index + 1).min(app.state.conversations.len().saturating_sub(1));
        }
        _ => {}
    }
}

fn handle_key(app: &mut App, key: KeyEvent, api: &LunaApi, actions: &mpsc::Sender<ActionResult>) {
    app.notice = None;
    if app.show_help {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
        ) {
            app.show_help = false;
        }
        return;
    }
    if app.confirm_interrupt {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                app.confirm_interrupt = false;
                if let Some(conversation_id) = app.state.selected_conversation_id {
                    spawn_abort(app, api, actions, conversation_id);
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => app.confirm_interrupt = false,
            _ => {}
        }
        return;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        let direction = match key.code {
            KeyCode::Char('h') => Some(FocusDirection::Left),
            KeyCode::Char('j') => Some(FocusDirection::Down),
            KeyCode::Char('k') => Some(FocusDirection::Up),
            KeyCode::Char('l') => Some(FocusDirection::Right),
            _ => None,
        };
        if let Some(direction) = direction {
            app.move_focus(direction);
            return;
        }
    }
    match key.code {
        KeyCode::Char('?') if app.focus != Focus::Composer => {
            app.show_help = true;
            return;
        }
        KeyCode::Tab | KeyCode::BackTab => {
            app.cycle_focus();
            return;
        }
        KeyCode::Esc => {
            app.focus = Focus::Transcript;
            app.error = None;
            return;
        }
        KeyCode::Char('q') if app.focus != Focus::Composer => {
            app.should_quit = true;
            return;
        }
        KeyCode::PageUp if app.focus != Focus::Composer => {
            load_earlier(app, api, actions);
            return;
        }
        _ => {}
    }

    match app.focus {
        Focus::List => handle_list_key(app, key, api, actions),
        Focus::Transcript => handle_transcript_key(app, key, api, actions),
        Focus::Composer => handle_composer_key(app, key, api, actions),
    }
}

fn handle_list_key(
    app: &mut App,
    key: KeyEvent,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.list_index = app.list_index.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.list_index =
                (app.list_index + 1).min(app.state.conversations.len().saturating_sub(1));
        }
        KeyCode::Enter if select_conversation(app, app.list_index, api, actions) => {
            app.focus = Focus::Transcript;
        }
        KeyCode::Char('n') => spawn_create(app, api, actions),
        _ => {}
    }
}

fn select_conversation(
    app: &mut App,
    index: usize,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
) -> bool {
    let Some(conversation_id) = app
        .state
        .conversations
        .get(index)
        .map(|conversation| conversation.id)
    else {
        return false;
    };
    app.state.select(conversation_id);
    app.transcript_offset_from_bottom = 0;
    if app.state.messages.contains_key(&conversation_id) {
        app.deferred_message_load = None;
    } else {
        app.deferred_message_load = Some(conversation_id);
        spawn_deferred_message_load(app, api, actions);
    }
    true
}

fn spawn_deferred_message_load(app: &mut App, api: &LunaApi, actions: &mpsc::Sender<ActionResult>) {
    if app.pending_action {
        return;
    }
    let Some(conversation_id) = app.deferred_message_load.take() else {
        return;
    };
    if app.state.selected_conversation_id == Some(conversation_id)
        && !app.state.messages.contains_key(&conversation_id)
    {
        spawn_load_messages(app, api, actions, conversation_id, None);
    }
}

fn handle_transcript_key(
    app: &mut App,
    key: KeyEvent,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.transcript_offset_from_bottom = app.transcript_offset_from_bottom.saturating_add(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.transcript_offset_from_bottom = app.transcript_offset_from_bottom.saturating_sub(1);
        }
        KeyCode::End => app.transcript_offset_from_bottom = 0,
        KeyCode::Char('i') | KeyCode::Enter => app.focus = Focus::Composer,
        KeyCode::Char('n') => spawn_create(app, api, actions),
        KeyCode::Char('s') if app.state.selected_conversation_id.is_some() => {
            app.confirm_interrupt = true;
        }
        KeyCode::PageUp => load_earlier(app, api, actions),
        _ => {}
    }
}

fn handle_composer_key(
    app: &mut App,
    key: KeyEvent,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
) {
    if app.pending_action {
        return;
    }
    if app.composer.handle_key(key) == ComposerAction::Submit
        && let Some(conversation_id) = app.state.selected_conversation_id
    {
        let text = app.composer.text().trim().to_owned();
        spawn_send(app, api, actions, conversation_id, text);
    }
}

fn load_earlier(app: &mut App, api: &LunaApi, actions: &mpsc::Sender<ActionResult>) {
    let Some(conversation_id) = app.state.selected_conversation_id else {
        return;
    };
    let Some(before) = app.state.next_before_ordinal.get(&conversation_id).copied() else {
        app.notice = Some("No earlier messages.".into());
        return;
    };
    spawn_load_messages(app, api, actions, conversation_id, Some(before));
}

fn handle_realtime_update(
    app: &mut App,
    update: RealtimeUpdate,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
) {
    match update {
        RealtimeUpdate::Connecting => app.connection = ConnectionStatus::Connecting,
        RealtimeUpdate::Connected => {
            app.connection = ConnectionStatus::Connected;
            app.connection_detail = None;
        }
        RealtimeUpdate::Disconnected(detail) => {
            app.connection = ConnectionStatus::Waiting;
            app.connection_detail = Some(detail);
        }
        RealtimeUpdate::Event(event) => match app.state.apply(*event) {
            StateEffect::None => app.sync_list_index(),
            StateEffect::Error(message) => app.error = Some(message),
            StateEffect::ResetRequired => {
                app.reset_required = true;
                if !app.pending_action {
                    app.reset_required = false;
                    spawn_reload(app, api, actions);
                }
            }
        },
    }
}

fn handle_action_result(
    app: &mut App,
    result: ActionResult,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
) {
    app.pending_action = false;
    match result {
        ActionResult::Messages {
            conversation_id,
            result,
        } => match result {
            Ok(page) => app.state.set_messages(conversation_id, page),
            Err(error) => app.error = Some(error),
        },
        ActionResult::Created(result) => match *result {
            Ok(conversation) => {
                let id = conversation.id;
                app.state.upsert_conversation(conversation);
                app.state.select(id);
                app.sync_list_index();
                app.focus = Focus::Composer;
                app.notice = Some("Conversation created.".into());
            }
            Err(error) => app.error = Some(error),
        },
        ActionResult::Sent(result) => match *result {
            Ok(response) => {
                app.state.upsert_message(response.message);
                app.composer.clear();
                app.focus = Focus::Transcript;
                app.transcript_offset_from_bottom = 0;
            }
            Err(error) => app.error = Some(error),
        },
        ActionResult::Aborted(result) => match result {
            Ok(()) => app.notice = Some("Interrupt requested.".into()),
            Err(error) => app.error = Some(error),
        },
        ActionResult::Reloaded(result) => match *result {
            Ok(bootstrap) => {
                app.reset_required = false;
                app.state.install(bootstrap);
                app.sync_list_index();
                if let Some(conversation_id) = app.state.selected_conversation_id {
                    spawn_load_messages(app, api, actions, conversation_id, None);
                }
            }
            Err(error) => app.error = Some(error),
        },
    }
    if app.reset_required && !app.pending_action {
        app.reset_required = false;
        spawn_reload(app, api, actions);
    }
    spawn_deferred_message_load(app, api, actions);
}

fn spawn_load_messages(
    app: &mut App,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
    conversation_id: Uuid,
    before: Option<i64>,
) {
    if app.pending_action {
        return;
    }
    app.pending_action = true;
    let api = api.clone();
    let actions = actions.clone();
    tokio::spawn(async move {
        let result = api
            .messages(conversation_id, before)
            .await
            .map_err(|error| error.to_string());
        let _ = actions
            .send(ActionResult::Messages {
                conversation_id,
                result,
            })
            .await;
    });
}

fn spawn_create(app: &mut App, api: &LunaApi, actions: &mpsc::Sender<ActionResult>) {
    if app.pending_action {
        return;
    }
    app.pending_action = true;
    let api = api.clone();
    let actions = actions.clone();
    tokio::spawn(async move {
        let result = api
            .create_conversation()
            .await
            .map_err(|error| error.to_string());
        let _ = actions.send(ActionResult::Created(Box::new(result))).await;
    });
}

fn spawn_send(
    app: &mut App,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
    conversation_id: Uuid,
    text: String,
) {
    app.pending_action = true;
    let api = api.clone();
    let actions = actions.clone();
    tokio::spawn(async move {
        let result = api
            .send_message(conversation_id, text)
            .await
            .map_err(|error| error.to_string());
        let _ = actions.send(ActionResult::Sent(Box::new(result))).await;
    });
}

fn spawn_abort(
    app: &mut App,
    api: &LunaApi,
    actions: &mpsc::Sender<ActionResult>,
    conversation_id: Uuid,
) {
    if app.pending_action {
        return;
    }
    app.pending_action = true;
    let api = api.clone();
    let actions = actions.clone();
    tokio::spawn(async move {
        let result = api
            .abort_conversation(conversation_id)
            .await
            .map_err(|error| error.to_string());
        let _ = actions.send(ActionResult::Aborted(result)).await;
    });
}

fn spawn_reload(app: &mut App, api: &LunaApi, actions: &mpsc::Sender<ActionResult>) {
    if app.pending_action {
        return;
    }
    app.pending_action = true;
    let api = api.clone();
    let actions = actions.clone();
    tokio::spawn(async move {
        let result = api.bootstrap().await.map_err(|error| error.to_string());
        let _ = actions.send(ActionResult::Reloaded(Box::new(result))).await;
    });
}

enum ActionResult {
    Messages {
        conversation_id: Uuid,
        result: Result<ConversationMessages, String>,
    },
    Created(Box<Result<Conversation, String>>),
    Sent(Box<Result<SendMessageResponse, String>>),
    Aborted(Result<(), String>),
    Reloaded(Box<Result<Bootstrap, String>>),
}

async fn termination_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler");
        let mut hangup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
            .expect("SIGHUP handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
            _ = hangup.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("terminal I/O failed: {0}")]
    Terminal(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;

    use super::*;
    use crate::api::ServerOrigin;

    #[test]
    fn mouse_clicks_focus_panels_and_the_wheel_scrolls_content() {
        let mut bootstrap = bootstrap();
        let mut second = bootstrap.conversations[0].clone();
        second.id = Uuid::from_u128(3);
        second.title = "Second conversation".into();
        bootstrap.conversations.push(second);
        let mut app = App::new(bootstrap);
        let target = app.state.conversations[1].id;
        app.state.messages.insert(target, Vec::new());
        app.focus = Focus::Composer;
        let regions = ui::UiRegions {
            list: Some(Rect::new(0, 0, 31, 30)),
            conversations: vec![ui::ConversationRegion {
                index: 1,
                area: Rect::new(1, 4, 29, 3),
            }],
            transcript: Some(Rect::new(31, 4, 69, 20)),
            composer: Some(Rect::new(31, 24, 69, 5)),
        };
        let api = LunaApi::new(
            ServerOrigin::parse("http://127.0.0.1:9").expect("origin"),
            Some("token".into()),
        )
        .expect("API");
        let (actions, _receiver) = mpsc::channel(1);

        click(&mut app, &regions, 2, 4, &api, &actions);
        assert_eq!(app.focus, Focus::List);
        assert_eq!(app.list_index, 1);
        assert_eq!(app.state.selected_conversation_id, Some(target));

        click(&mut app, &regions, 40, 5, &api, &actions);
        assert_eq!(app.focus, Focus::Transcript);
        mouse(
            &mut app,
            &regions,
            MouseEventKind::ScrollUp,
            40,
            5,
            &api,
            &actions,
        );
        assert_eq!(app.transcript_offset_from_bottom, MOUSE_SCROLL_LINES);
        assert_eq!(app.focus, Focus::Transcript);
        mouse(
            &mut app,
            &regions,
            MouseEventKind::ScrollLeft,
            40,
            5,
            &api,
            &actions,
        );
        assert_eq!(app.transcript_offset_from_bottom, MOUSE_SCROLL_LINES);
        mouse(
            &mut app,
            &regions,
            MouseEventKind::ScrollDown,
            40,
            5,
            &api,
            &actions,
        );
        assert_eq!(app.transcript_offset_from_bottom, 0);

        click(&mut app, &regions, 40, 25, &api, &actions);
        assert_eq!(app.focus, Focus::Composer);
        mouse(
            &mut app,
            &regions,
            MouseEventKind::ScrollUp,
            2,
            10,
            &api,
            &actions,
        );
        assert_eq!(app.focus, Focus::List);
        assert_eq!(app.list_index, 0);
        mouse(
            &mut app,
            &regions,
            MouseEventKind::ScrollDown,
            2,
            10,
            &api,
            &actions,
        );
        assert_eq!(app.list_index, 1);
        assert_eq!(app.state.selected_conversation_id, Some(target));

        app.show_help = true;
        click(&mut app, &regions, 40, 25, &api, &actions);
        assert_eq!(app.focus, Focus::List);
    }

    #[tokio::test]
    async fn defers_a_selected_conversation_load_until_the_current_action_finishes() {
        let mut app = App::new(bootstrap());
        let conversation_id = app.state.selected_conversation_id.expect("selection");
        app.pending_action = true;
        let api = LunaApi::new(
            ServerOrigin::parse("http://127.0.0.1:9").expect("origin"),
            Some("token".into()),
        )
        .expect("API");
        let (actions, mut receiver) = mpsc::channel(1);

        assert!(select_conversation(&mut app, 0, &api, &actions));
        assert_eq!(app.deferred_message_load, Some(conversation_id));

        app.pending_action = false;
        spawn_deferred_message_load(&mut app, &api, &actions);
        assert!(app.pending_action);
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await,
            Ok(Some(ActionResult::Messages {
                conversation_id: loaded,
                result: Err(_),
            })) if loaded == conversation_id
        ));
    }

    #[test]
    fn starts_with_the_conversation_list_focused() {
        assert_eq!(App::new(bootstrap()).focus, Focus::List);
    }

    #[test]
    fn control_hjkl_moves_focus_directionally() {
        let mut app = App::new(bootstrap());

        press(&mut app, KeyCode::Char('l'), KeyModifiers::CONTROL);
        assert_eq!(app.focus, Focus::Transcript);
        press(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert_eq!(app.focus, Focus::Composer);
        press(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(app.focus, Focus::Transcript);
        press(&mut app, KeyCode::Char('h'), KeyModifiers::CONTROL);
        assert_eq!(app.focus, Focus::List);

        press(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(app.focus, Focus::List);
        press(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert_eq!(app.focus, Focus::List);
    }

    #[test]
    fn unmodified_hjkl_keep_their_panel_actions() {
        let mut app = App::new(bootstrap());
        app.focus = Focus::Transcript;
        press(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(app.transcript_offset_from_bottom, 1);
        assert_eq!(app.focus, Focus::Transcript);

        app.focus = Focus::Composer;
        press(&mut app, KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(app.composer.text(), "h");
        assert_eq!(app.focus, Focus::Composer);
    }

    #[test]
    fn quitting_schedules_no_interrupt_action() {
        let mut app = App::new(bootstrap());
        let api = LunaApi::new(
            ServerOrigin::parse("http://127.0.0.1:9").expect("origin"),
            Some("token".into()),
        )
        .expect("API");
        let (actions, mut receiver) = mpsc::channel(1);

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            &api,
            &actions,
        );

        assert!(app.should_quit);
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    fn click(
        app: &mut App,
        regions: &ui::UiRegions,
        column: u16,
        row: u16,
        api: &LunaApi,
        actions: &mpsc::Sender<ActionResult>,
    ) {
        mouse(
            app,
            regions,
            MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            api,
            actions,
        );
    }

    fn mouse(
        app: &mut App,
        regions: &ui::UiRegions,
        kind: MouseEventKind,
        column: u16,
        row: u16,
        api: &LunaApi,
        actions: &mpsc::Sender<ActionResult>,
    ) {
        handle_terminal_event(
            app,
            Event::Mouse(MouseEvent {
                kind,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            }),
            regions,
            api,
            actions,
        );
    }

    fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
        let api = LunaApi::new(
            ServerOrigin::parse("http://127.0.0.1:9").expect("origin"),
            Some("token".into()),
        )
        .expect("API");
        let (actions, _receiver) = mpsc::channel(1);
        handle_key(app, KeyEvent::new(code, modifiers), &api, &actions);
    }

    fn bootstrap() -> Bootstrap {
        serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "cursor": 1,
            "device": {
                "id": "00000000-0000-0000-0000-000000000001",
                "name": "Terminal",
                "platform": "tui",
                "notificationsEnabled": false,
                "createdAt": "2026-01-01T00:00:00Z",
                "lastSeenAt": "2026-01-01T00:00:00Z"
            },
            "conversations": [{
                "id": "00000000-0000-0000-0000-000000000002",
                "title": "Conversation",
                "titleMode": "automatic",
                "state": "working",
                "preview": "",
                "activeWorkingDirectory": "/tmp",
                "repositories": [],
                "activities": [],
                "unreadCount": 0,
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z",
                "version": 1
            }]
        }))
        .expect("bootstrap")
    }
}
