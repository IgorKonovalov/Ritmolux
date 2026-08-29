# The pinned Spout SDK. THIS FILE IS THE BUMP (Plan 0115, ADR-0125).
#
# Dot-sourced by fetch-sdk.ps1, which downloads and checks it. The same shape as
# packaging/foobar/sdk-pin.ps1 and for the same reason: moving the SDK is one
# edit here rather than several that can disagree.
#
# PINNED, NOT LATEST - the trade ADR-0115 records for the foobar SDK applies
# unchanged. Tracking latest would mean lmv.exe silently rebuilt against an SDK
# nobody sent a frame through. Pinning instead means the SDK can go stale, and
# the thing that finds out is a person.
#
# WHAT THE BUILD ACTUALLY NEEDS is the SDK *binaries* archive below: the SpoutDX
# headers and the MD/MT static and import libraries. The C++ shim compiles
# against include/SpoutDX and links MD/lib/SpoutDX_static.lib.
#
# ONE VARIABLE NO TEST HERE CAN SEE. The receiver is versioned independently of
# us: the TouchDesigner install this was proven against (2025.33070) links Spout
# SDK 2.007.014 while we send with 2.007.017. That is expected to be harmless -
# Spout's cross-process contract is the shared-memory sender-name map plus a
# DX11 shared-texture handle, not the library version - but it is unverified by
# anything automatic. If a sender stops appearing in a receiver after a bump,
# suspect the skew before suspecting the shim.
#
# TO BUMP: change all three constants together, re-run
# packaging/spout/fetch-sdk.ps1 -Force, rebuild with --features spout, and put a
# frame into a real Syphon Spout In TOP before shipping. A bump is its own
# commit.

$LmvSpoutVersion = "2.007.017"
$LmvSpoutUrl = "https://github.com/leadedge/Spout2/releases/download/2.007.017/Spout-SDK-binaries_2-007-017_1.zip"
# Verified 2026-08-29 against the published GitHub release asset (3,472,666 bytes).
$LmvSpoutSha256 = "695f20e3505fa0da51b2eb959af359f5d9e2c914bb9676e9118d19f6a5424bf4"

# NOT a build input, and deliberately not fetched by fetch-sdk.ps1: the same
# release also publishes SPOUT_2007-017.zip (12,422,533 bytes, sha256
# 944f4ef7648a89087757bcbaaebd277bcdc47afc5af1435b5ed0f6298a74f8c7), which
# carries DEMO/SpoutSender.exe and DEMO/SpoutReceiver.exe. Those are the
# known-good sender and receiver a person points a TOP at when deciding whether
# a problem is ours or the transport's. Named here so the next person does not
# have to rediscover that they exist.
