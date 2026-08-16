# The pinned foobar2000 SDK. THIS FILE IS THE BUMP (Plan 0102, ADR-0115).
#
# Dot-sourced by fetch-sdk.ps1 (which downloads and checks it) and by
# build-component.ps1 (which stamps the version into what a recipient reads), so
# moving the SDK is one edit here rather than three edits that can disagree.
#
# PINNED, NOT LATEST - and this is a deliberate trade, so read it before
# bumping. Tracking latest would mean a component silently rebuilt against an
# SDK nobody tested, riding a version bump that says nothing about it
# (ADR-0115 Notes). Pinning instead means the SDK *can* go stale, and the thing
# that finds out is a person: nothing in this repository watches foobar2000's
# SDK changelog, and no CI runner can load foobar2000 to notice a component-ABI
# break. The three guards that do exist:
#
#   1. fetch-sdk.ps1 fails the release if the URL 404s or the bytes change.
#   2. Plan 0102 Phase 5's clean-profile install is the only functional check.
#   3. This comment, which the next person to bump reads.
#
# The pin does not rot on its own: foobar2000.org keeps every SDK release at a
# stable /downloads/SDK-<date>.7z URL back to 2011-03-11, so an old pin stays
# fetchable indefinitely.
#
# TO BUMP: change all three constants together, re-run
# packaging/foobar/fetch-sdk.ps1 -Force, rebuild, and re-run Plan 0102 Phase 5's
# on-device check before shipping. A bump is its own commit - never fold one
# into an unrelated change, which is the exact failure mode the pin prevents.

$LmvSdkVersion = "2025-03-07"
$LmvSdkUrl = "https://www.foobar2000.org/downloads/SDK-2025-03-07.7z"
# Verified 2026-08-16: the published archive is byte-identical to the copy this
# project has built against since Plan 0001.
$LmvSdkSha256 = "ccda3c5840e66e0e28a7e4fe36407c4e78581aa30c40c362a188fcbaae799a3e"
