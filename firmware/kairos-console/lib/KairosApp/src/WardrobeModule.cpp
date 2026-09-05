#include "WardrobeModule.h"

#include "Theme.h"

namespace kairos {

const char* WardrobeModule::id() const { return "wardrobe"; }

const char* WardrobeModule::title() const { return "WARDROBE"; }

const char* WardrobeModule::subtitle() const {
  return "OUTFIT MODULE // READY";
}

std::uint64_t WardrobeModule::revision() const { return 0; }

void WardrobeModule::enter() {}

void WardrobeModule::render(TFT_eSPI& display, bool full_redraw) {
  if (!full_redraw) {
    return;
  }
  display.fillScreen(theme::kBackground);
  display.setTextDatum(TL_DATUM);
  display.setTextColor(theme::kMuted, theme::kBackground);
  display.drawString("< HOME", 8, 13, 2);
  display.setTextColor(theme::kAccent, theme::kBackground);
  display.drawString("WARDROBE", 88, 13, 2);
  display.drawFastHLine(8, 45, 304, theme::kPanel);

  display.fillRoundRect(24, 72, 272, 108, 8, theme::kPanel);
  display.drawRoundRect(24, 72, 272, 108, 8, theme::kAccent);
  display.setTextDatum(TC_DATUM);
  display.setTextColor(theme::kAccent, theme::kPanel);
  display.drawString("MODULE SHELL READY", 160, 93, 2);
  display.setTextColor(theme::kMuted, theme::kPanel);
  display.drawString("OUTFIT DATA COMES NEXT", 160, 126, 2);
  display.drawString("ADD OUTFITS IN THE NEXT BUILD", 160, 151, 1);
  display.setTextDatum(TL_DATUM);
}

bool WardrobeModule::on_touch(const TouchPoint& point, std::uint32_t now_ms) {
  (void)point;
  (void)now_ms;
  return false;
}

bool WardrobeModule::on_tick(std::uint32_t now_ms) {
  (void)now_ms;
  return false;
}

void WardrobeModule::save_state() {}

AppCommandDecision WardrobeModule::apply_command(const AppCommand& command,
                                                 std::uint32_t now_ms) {
  (void)command;
  (void)now_ms;
  return {AppCommandError::UnsupportedAction, "wardrobe commands come later"};
}

bool WardrobeModule::take_command_result(AppCommandResult& result) {
  (void)result;
  return false;
}

void WardrobeModule::cancel_pending_command() {}

}  // namespace kairos
