use luna_pi::read_jsonl_record;
use serde_json::json;
use tokio::io::BufReader;

#[tokio::test]
async fn framing_splits_only_on_lf() {
    let input = b"{\"value\":\"before\xE2\x80\xA8after\"}\n{\"value\":2}\r\n";
    let mut reader = BufReader::new(&input[..]);
    let first = read_jsonl_record(&mut reader)
        .await
        .expect("first")
        .expect("record");
    let second = read_jsonl_record(&mut reader)
        .await
        .expect("second")
        .expect("record");
    assert_eq!(first, json!({ "value": "before\u{2028}after" }));
    assert_eq!(second, json!({ "value": 2 }));
    assert!(read_jsonl_record(&mut reader).await.expect("eof").is_none());
}
