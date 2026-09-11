import AppKit
import ApplicationServices
import Foundation

func attribute(_ node: AXUIElement, _ name: CFString) -> AnyObject? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(node, name, &value) == .success else { return nil }
    return value
}
func children(_ node: AXUIElement) -> [AXUIElement] {
    attribute(node, kAXChildrenAttribute as CFString) as? [AXUIElement] ?? []
}
func string(_ node: AXUIElement, _ name: CFString) -> String {
    attribute(node, name) as? String ?? ""
}
func rectangle(_ node: AXUIElement) -> CGRect? {
    guard let position = attribute(node, kAXPositionAttribute as CFString),
          let size = attribute(node, kAXSizeAttribute as CFString),
          CFGetTypeID(position) == AXValueGetTypeID(), CFGetTypeID(size) == AXValueGetTypeID() else { return nil }
    var point = CGPoint.zero, dimensions = CGSize.zero
    guard AXValueGetValue(unsafeBitCast(position, to: AXValue.self), .cgPoint, &point),
          AXValueGetValue(unsafeBitCast(size, to: AXValue.self), .cgSize, &dimensions) else { return nil }
    return CGRect(origin: point, size: dimensions)
}
func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8)); exit(1)
}
guard CommandLine.arguments.count == 4, let pid = Int32(CommandLine.arguments[1]) else {
    fail("usage: native-menu PID inspect|hover|click title")
}
if let session = CGSessionCopyCurrentDictionary() as? [String: Any],
   (session["CGSSessionScreenIsLocked"] as? Bool) == true {
    fail("macOS session is locked; unlock it to run native menu input")
}
guard AXIsProcessTrusted() else { fail("native menu driver lacks Accessibility permission") }
let operation = CommandLine.arguments[2], title = CommandLine.arguments[3]
guard ["inspect", "hover", "click", "activate"].contains(operation) else { fail("unsupported native operation") }
if operation == "activate" {
    guard let application = NSRunningApplication(processIdentifier: pid) else { fail("test-owned application exited") }
    application.activate(options: [.activateIgnoringOtherApps])
    let deadline = Date().addingTimeInterval(2)
    while NSWorkspace.shared.frontmostApplication?.processIdentifier != pid && Date() < deadline {
        RunLoop.current.run(until: Date().addingTimeInterval(0.01))
    }
}
guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
    fail("native menu action requires test PID \(pid) in front; observed \(NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1)")
}
if operation == "activate" { print("{\"activated\":true}"); exit(0) }
let app = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(app, 2)
var queue: [(AXUIElement, Int, Bool)] = [(app, 0, false)], matches: [AXUIElement] = []
var visited = 0, seen: [String] = []
while !queue.isEmpty && visited < 2500 {
    let (node, depth, menuParent) = queue.removeFirst(); visited += 1
    let role = string(node, kAXRoleAttribute as CFString)
    let name = string(node, kAXTitleAttribute as CFString)
    if role == "AXMenuItem" && menuParent {
        seen.append(name)
        if name == title { matches.append(node) }
    }
    // Native popup menus are separate from the application menu bar. Do not
    // explore or operate menu-bar actions, web content, or another process.
    if depth < 9 && role != "AXMenuBar" && role != "AXWebArea" {
        queue += children(node).map { ($0, depth + 1, menuParent || role == "AXMenu") }
    }
}
guard matches.count == 1, let rect = rectangle(matches[0]), rect.width > 0, rect.height > 0 else {
    fail("expected one visible native menu item '\(title)'; found \(matches.count), menu items: \(seen)")
}
let item = matches[0]
let enabled = attribute(item, kAXEnabledAttribute as CFString) as? Bool ?? false
guard enabled else { fail("native menu item is disabled: \(title)") }
let point = CGPoint(x: rect.midX, y: rect.midY)
if operation != "inspect" {
    let events: [CGEventType] = operation == "hover" ? [.mouseMoved] : [.mouseMoved, .leftMouseDown, .leftMouseUp]
    for type in events {
        guard let event = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .left) else {
            fail("could not create native mouse event")
        }
        event.postToPid(pid)
    }
}
var menu = item
for _ in 0..<4 {
    if string(menu, kAXRoleAttribute as CFString) == "AXMenu" { break }
    guard let parent = attribute(menu, kAXParentAttribute as CFString) else { break }
    menu = unsafeBitCast(parent, to: AXUIElement.self)
}
let bounds = rectangle(menu) ?? rect
let result: [String: Any] = ["title": title, "role": "AXMenuItem", "enabled": enabled,
    "pid": pid, "operation": operation,
    "point": ["x": point.x, "y": point.y],
    "menuBounds": ["x": bounds.minX, "y": bounds.minY, "width": bounds.width, "height": bounds.height]]
let data = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
FileHandle.standardOutput.write(data)
