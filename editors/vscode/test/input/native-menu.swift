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
guard ["inspect", "hover", "click", "activate", "context-click"].contains(operation) else { fail("unsupported native operation") }
if operation == "activate" {
    guard let application = NSRunningApplication(processIdentifier: pid) else { fail("test-owned application exited") }
    application.activate(options: [])
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
if operation == "context-click" {
    guard let data = title.data(using: .utf8),
          let target = try JSONSerialization.jsonObject(with: data) as? [String: Double],
          let x = target["x"], let y = target["y"], let width = target["width"], let height = target["height"],
          let window = attribute(app, kAXFocusedWindowAttribute as CFString),
          let bounds = rectangle(unsafeBitCast(window, to: AXUIElement.self)),
          abs(bounds.width - width) < 2, bounds.height >= height,
          x >= 0, x < width, y >= 0, y < height else { fail("context pointer does not match the focused test window") }
    let point = CGPoint(x: bounds.minX + x, y: bounds.maxY - height + y)
    for type in [CGEventType.mouseMoved, .rightMouseDown, .rightMouseUp] {
        guard let event = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .right) else {
            fail("could not create native context click")
        }
        event.flags = []
        event.setIntegerValueField(.mouseEventClickState, value: 1)
        event.post(tap: .cghidEventTap)
        RunLoop.current.run(until: Date().addingTimeInterval(0.03))
    }
    let result: [String: Any] = ["pid": pid, "operation": operation, "point": ["x": point.x, "y": point.y]]
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: result)); exit(0)
}
var queue: [(AXUIElement, Int, Bool)] = [(app, 0, false)], matches: [AXUIElement] = []
var focusedDescription = "none"
if let focused = attribute(app, kAXFocusedUIElementAttribute as CFString) {
    let element = unsafeBitCast(focused, to: AXUIElement.self)
    focusedDescription = string(element, kAXRoleAttribute as CFString) + ":" + string(element, kAXTitleAttribute as CFString)
    queue.insert((element, 0, true), at: 0)
    var parent = element
    for _ in 0..<5 {
        guard let value = attribute(parent, kAXParentAttribute as CFString) else { break }
        parent = unsafeBitCast(value, to: AXUIElement.self)
        if string(parent, kAXRoleAttribute as CFString) == "AXMenu" {
            queue.insert((parent, 0, true), at: 0); break
        }
    }
}
var visited = 0, seen: [String] = [], roles: [String] = [], processed: [AXUIElement] = []
while !queue.isEmpty && visited < 2500 {
    let (node, depth, menuParent) = queue.removeFirst(); visited += 1
    if processed.contains(where: { CFEqual($0, node) }) { continue }
    processed.append(node)
    let role = string(node, kAXRoleAttribute as CFString)
    let name = string(node, kAXTitleAttribute as CFString)
    if depth < 4 { roles.append("\(depth):\(role):\(name)") }
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
    fail("expected one visible native menu item '\(title)'; found \(matches.count), menu items: \(seen), focused: \(focusedDescription), roots: \(roles)")
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
        event.flags = []
        event.setIntegerValueField(.mouseEventClickState, value: 1)
        event.post(tap: .cghidEventTap)
        RunLoop.current.run(until: Date().addingTimeInterval(0.03))
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
