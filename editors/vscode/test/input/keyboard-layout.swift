import Carbon
import Foundation

func identifier(_ source: TISInputSource) -> String {
    guard let value = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) else {
        fatalError("input source has no identifier")
    }
    return Unmanaged<CFString>.fromOpaque(value).takeUnretainedValue() as String
}
let arguments = CommandLine.arguments
if arguments.count == 2 && arguments[1] == "get" {
    print(identifier(TISCopyCurrentKeyboardInputSource().takeRetainedValue()))
} else if arguments.count == 3 && arguments[1] == "select" {
    let filter = [kTISPropertyInputSourceID as String: arguments[2]] as CFDictionary
    let sources = TISCreateInputSourceList(filter, false).takeRetainedValue() as! [TISInputSource]
    guard sources.count == 1, TISSelectInputSource(sources[0]) == noErr else {
        FileHandle.standardError.write(Data("requested input source is unavailable\n".utf8)); exit(1)
    }
    guard identifier(TISCopyCurrentKeyboardInputSource().takeRetainedValue()) == arguments[2] else { exit(1) }
} else {
    FileHandle.standardError.write(Data("usage: keyboard-layout get|select identifier\n".utf8)); exit(1)
}
