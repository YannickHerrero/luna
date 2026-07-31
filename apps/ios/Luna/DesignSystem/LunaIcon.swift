import SwiftUI

enum LunaIcon: String, CaseIterable, Sendable {
    case archive = "LucideArchive"
    case arrowLeft = "LucideArrowLeft"
    case camera = "LucideCamera"
    case check = "LucideCheck"
    case chevronDown = "LucideChevronDown"
    case circle = "LucideCircle"
    case circleStop = "LucideCircleStop"
    case listChecks = "LucideListChecks"
    case mic = "LucideMic"
    case minus = "LucideMinus"
    case moon = "LucideMoon"
    case paperclip = "LucidePaperclip"
    case plus = "LucidePlus"
    case search = "LucideSearch"
    case send = "LucideSend"
    case settings = "LucideSettings"
    case sun = "LucideSun"
    case triangleAlert = "LucideTriangleAlert"
    case x = "LucideX"
}

struct LunaIconView: View {
    let icon: LunaIcon
    var size: CGFloat = 18

    var body: some View {
        Image(icon.rawValue)
            .resizable()
            .renderingMode(.template)
            .scaledToFit()
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}
