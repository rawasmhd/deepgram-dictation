# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

## What is signed

Only `dictation.exe` from the [releases](https://github.com/rawasmhd/deepgram-dictation/releases) of this repository. GitHub Actions builds it from the source code in this repository (`.github/workflows/release.yml`). Nothing built elsewhere is signed.

## Team roles

| Role | Members |
|---|---|
| Committers (can change the code without more review) | [rawasmhd](https://github.com/rawasmhd) |
| Reviewers (approve all changes from other contributors) | [rawasmhd](https://github.com/rawasmhd) |
| Approvers (approve each signing request) | [rawasmhd](https://github.com/rawasmhd) |

Every release needs a manual approval before it is signed. All team members use multi-factor authentication on GitHub and SignPath.

## Privacy

See the [privacy policy](PRIVACY.md). In short: the app sends your microphone audio to Deepgram for transcription only while you record, and sends nothing else.
