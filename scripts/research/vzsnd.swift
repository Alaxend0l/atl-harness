// SPDX-License-Identifier: AGPL-3.0-or-later
// andrun-vz spike: minimal VZ launcher with virtio-sound (proof for
// docs/research/vm-launcher-distribution.md evidence step 3).
import Foundation
import Virtualization

final class Handler: NSObject, VZVirtualMachineDelegate {
    func guestDidStop(_ vm: VZVirtualMachine) { print("guest stopped"); exit(0) }
    func virtualMachine(_ vm: VZVirtualMachine, didStopWithError error: Error) {
        print("stopped with error: \(error)"); exit(1)
    }
    func virtualMachine(_ vm: VZVirtualMachine, networkDevice d: VZNetworkDevice,
                        attachmentWasDisconnectedWithError error: Error) {
        print("net error: \(error)")
    }
}

let dir = FileManager.default.currentDirectoryPath
let config = VZVirtualMachineConfiguration()
config.cpuCount = 4
config.memorySize = 4 << 30
let efi = VZEFIBootLoader()
efi.variableStore = try! VZEFIVariableStore(url: URL(fileURLWithPath: "\(dir)/efivars.bin"))
config.bootLoader = efi
config.platform = VZGenericPlatformConfiguration()
let diskAtt = try! VZDiskImageStorageDeviceAttachment(url: URL(fileURLWithPath: "\(dir)/disk.raw"), readOnly: false)
config.storageDevices = [VZVirtioBlockDeviceConfiguration(attachment: diskAtt)]
let net = VZVirtioNetworkDeviceConfiguration()
net.attachment = VZNATNetworkDeviceAttachment()
net.macAddress = VZMACAddress(string: "52:54:00:41:54:4c")!
config.networkDevices = [net]
config.entropyDevices = [VZVirtioEntropyDeviceConfiguration()]
let serialCfg = VZVirtioConsoleDeviceSerialPortConfiguration()
let fh = FileHandle(forWritingAtPath: "\(dir)/serial3.log")!
try! fh.seekToEnd()
serialCfg.attachment = VZFileHandleSerialPortAttachment(fileHandleForReading: nil, fileHandleForWriting: fh)
config.serialPorts = [serialCfg]

// The point of the spike: a virtio-sound output stream to the host default device.
if #available(macOS 15.0, *) {
    let sound = VZVirtioSoundDeviceConfiguration()
    let out = VZVirtioSoundDeviceOutputStreamConfiguration()
    out.sink = VZHostAudioOutputStreamSink()
    sound.streams = [out]
    config.audioDevices = [sound]
} else {
    fatalError("needs macOS 15+")
}

try! config.validate()
let vm = VZVirtualMachine(configuration: config)
let h = Handler()
vm.delegate = h
vm.start { r in
    if case .failure(let e) = r { FileHandle.standardError.write("start failed: \(e)\n".data(using: .utf8)!); exit(1) }
    FileHandle.standardError.write("VM RUNNING\n".data(using: .utf8)!)
}
RunLoop.main.run()
