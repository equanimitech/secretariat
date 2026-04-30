// touchid-prompt — Touch ID biometric gate for the Secretariat CLI.
//
// Usage: touchid-prompt "<reason string shown in the system prompt>"
// Exit codes:
//   0  — biometric verified
//   1  — biometric refused or cancelled
//   2  — biometric not available on this machine

import Foundation
import LocalAuthentication

let reason = CommandLine.arguments.count > 1
    ? CommandLine.arguments[1]
    : "Authenticate to stamp a Secretariat envelope"

let context = LAContext()
context.localizedFallbackTitle = "" // hide password fallback

var availabilityError: NSError?
guard context.canEvaluatePolicy(
    .deviceOwnerAuthenticationWithBiometrics,
    error: &availabilityError
) else {
    let msg = availabilityError?.localizedDescription ?? "biometric unavailable"
    FileHandle.standardError.write(("touchid-prompt: \(msg)\n").data(using: .utf8)!)
    exit(2)
}

let semaphore = DispatchSemaphore(value: 0)
var success = false

context.evaluatePolicy(
    .deviceOwnerAuthenticationWithBiometrics,
    localizedReason: reason
) { ok, err in
    success = ok
    if let err = err {
        FileHandle.standardError.write(
            ("touchid-prompt: \(err.localizedDescription)\n").data(using: .utf8)!
        )
    }
    semaphore.signal()
}
semaphore.wait()

exit(success ? 0 : 1)
