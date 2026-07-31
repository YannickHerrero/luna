# Luna TestFlight delivery

Luna uses Fastlane with an App Store Connect API key stored outside the repository. The release archive contains the universal iPhone/iPad app, its WidgetKit extension, the embedded Apple Watch companion app, and the Watch widget extension.

## One-time local setup

1. Copy `fastlane/.env.example` to the ignored `fastlane/.env`.
2. Set the App Store Connect key ID, issuer ID, and Apple Developer team ID.
3. Store the private key at `~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8` with mode `600`, or set `ASC_KEY_PATH`.
4. Copy `apps/ios/Config/Local.xcconfig.example` to the ignored `apps/ios/Config/Local.xcconfig` for physical-device development signing.

Never commit API keys, private keys, certificates, provisioning profiles, or local signing metadata.

## Lanes

```sh
fastlane ios bootstrap # register IDs, Push capability, and the app record
fastlane ios signing   # create/download all four App Store profiles
fastlane ios build     # generate, archive, and export fastlane/build/Luna.ipa
fastlane ios upload    # upload an existing IPA and wait for processing
fastlane ios beta      # build and upload
```

`bootstrap` attempts to create the App Store Connect app record. If Apple requires manual creation, follow the exact instructions printed by the lane and rerun it. The beta lane targets internal TestFlight only; external testing and App Store submission remain separate actions.

Push capability is reserved for the main application, but Luna does not request notification permission or register an APNs provider until the server registration and delivery design is implemented. The widget and Watch surfaces intentionally communicate that live status is coming soon.
