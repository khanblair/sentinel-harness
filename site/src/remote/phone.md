# Phone over Tailscale

## What it is

The phone app is the same web interface as the PC, installed on your phone and reached over Tailscale.

## Steps

1. Install Tailscale on the PC and on the phone. Sign in to the same account on both.
2. Open Remote on the PC. Sentinel first checks that Tailscale is installed, running, and signed in. If a check fails, it shows install steps or a sign-in button.
3. Scan the QR code with the phone, or open the link on the phone. The PC shows the machine name and a one-time code.
4. Enter the one-time code on the phone. Approve the device on the PC. The phone then receives a device token.
5. Add the app to the home screen. Use the bottom tabs to move between Stream, Board, Reserved, Changes, and More.

## Screenshot

![Screenshot of remote-pairing](../images/remote-pairing.png)

## Tip

Pairing codes expire after a short time, and each code works once.

## Related

- [What each remote may do](remote-limits.md)
- [Panels](../using/panels.md)
- [Always-on machine](../get-started/always-on-machine.md)
