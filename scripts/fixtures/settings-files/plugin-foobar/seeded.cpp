// Seeded for `check-settings-have-files.mjs`. One declaration the fixture's
// document names and one it does not, so the gate's verdict turns on the claim
// rather than on the declaration.

namespace rlx {

namespace {

constexpr GUID kGuidSeeded = {
    0x11111111, 0x2222, 0x3333, {0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb}};

constexpr GUID kGuidDocumented = {
    0xcccccccc, 0xdddd, 0xeeee, {0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08}};

// Unclaimed: no document names it, so nobody can tell a setting from resume
// state and the gate reports it.
cfg_int g_cfg_seeded(kGuidSeeded, 0);

// Claimed: the fixture's document names it in backticks and says what it is.
cfg_string g_cfg_documented(kGuidDocumented, "");

} // namespace

int seeded_value() { return g_cfg_seeded.get(); }

} // namespace rlx
