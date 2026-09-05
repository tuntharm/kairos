#pragma once

#include <cstddef>
#include <cstdint>

namespace kairos {

constexpr int kHomeWidth = 320;
constexpr int kHomeHeight = 240;
constexpr int kLeftPaneWidth = 128;
constexpr int kRightPaneX = 128;
constexpr int kRightPaneWidth = 192;
constexpr int kHomeHeaderHeight = 28;
constexpr int kVisibleAppRows = 4;
constexpr int kAppRowHeight = 36;
constexpr int kAppListX = 136;
constexpr int kAppListY = 32;
constexpr int kAppListWidth = 156;
constexpr int kChevronX = 296;
constexpr int kChevronWidth = 22;
constexpr int kStatusX = 132;
constexpr int kStatusY = 176;
constexpr int kStatusWidth = 184;
constexpr int kStatusHeight = 60;

struct HomeTouchResult {
  enum class Kind : std::uint8_t { None, OpenModule, ScrollUp, ScrollDown };

  Kind kind{Kind::None};
  std::size_t module_index{0};
};

inline bool home_inside(std::uint16_t x, std::uint16_t y, int left, int top,
                        int width, int height) {
  return x >= left && x < left + width && y >= top && y < top + height;
}

inline std::size_t max_home_scroll(std::size_t module_count) {
  if (module_count <= static_cast<std::size_t>(kVisibleAppRows)) {
    return 0;
  }
  return module_count - static_cast<std::size_t>(kVisibleAppRows);
}

inline std::size_t clamp_home_scroll(std::size_t scroll,
                                     std::size_t module_count) {
  const std::size_t maximum = max_home_scroll(module_count);
  return scroll > maximum ? maximum : scroll;
}

inline bool home_status_contains(std::uint16_t x, std::uint16_t y,
                                 bool status_active) {
  return status_active &&
         home_inside(x, y, kStatusX, kStatusY, kStatusWidth, kStatusHeight);
}

inline HomeTouchResult hit_home(std::uint16_t x, std::uint16_t y,
                                std::size_t module_count, std::size_t scroll,
                                bool status_active) {
  if (home_status_contains(x, y, status_active)) {
    return {};
  }

  const std::size_t maximum = max_home_scroll(module_count);
  if (maximum > 0) {
    if (home_inside(x, y, kChevronX, kAppListY, kChevronWidth, 28) &&
        scroll > 0) {
      return {HomeTouchResult::Kind::ScrollUp, 0};
    }
    if (home_inside(x, y, kChevronX, kAppListY + 108, kChevronWidth, 28) &&
        scroll < maximum) {
      return {HomeTouchResult::Kind::ScrollDown, 0};
    }
  }

  const std::size_t visible = module_count < static_cast<std::size_t>(kVisibleAppRows)
                                  ? module_count
                                  : static_cast<std::size_t>(kVisibleAppRows);
  for (std::size_t row = 0; row < visible; ++row) {
    const int top = kAppListY + static_cast<int>(row) * kAppRowHeight;
    if (home_inside(x, y, kAppListX, top, kAppListWidth, kAppRowHeight - 4)) {
      return {HomeTouchResult::Kind::OpenModule, scroll + row};
    }
  }
  return {};
}

}  // namespace kairos
