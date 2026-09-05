#include <unity.h>

#include "CharacterProgress.h"
#include "HomeLayout.h"
#include "PomodoroModel.h"

using kairos::character_progress;
using kairos::clamp_home_scroll;
using kairos::hit_home;
using kairos::HomeTouchResult;
using kairos::kAppListX;
using kairos::kAppListY;
using kairos::kChevronX;
using kairos::kStatusX;
using kairos::kStatusY;
using kairos::max_home_scroll;
using kairos::PomodoroModel;
using kairos::PomodoroPreset;
using kairos::PomodoroSegment;
using kairos::PomodoroStatus;

void test_rank_starts_at_one() {
  const auto progress = character_progress(0);
  TEST_ASSERT_EQUAL_UINT32(1, progress.rank);
  TEST_ASSERT_EQUAL_UINT32(0, progress.xp_in_rank);
}

void test_rank_advances_every_four_sessions() {
  const auto mid = character_progress(3);
  TEST_ASSERT_EQUAL_UINT32(1, mid.rank);
  TEST_ASSERT_EQUAL_UINT32(3, mid.xp_in_rank);

  const auto next = character_progress(4);
  TEST_ASSERT_EQUAL_UINT32(2, next.rank);
  TEST_ASSERT_EQUAL_UINT32(0, next.xp_in_rank);
}

void test_home_scroll_clamps() {
  TEST_ASSERT_EQUAL_UINT32(0, max_home_scroll(2));
  TEST_ASSERT_EQUAL_UINT32(1, max_home_scroll(5));
  TEST_ASSERT_EQUAL_UINT32(1, clamp_home_scroll(9, 5));
}

void test_name_row_opens_visible_module() {
  const HomeTouchResult hit =
      hit_home(kAppListX + 20, kAppListY + 8, 2, 0, false);
  TEST_ASSERT_EQUAL(static_cast<int>(HomeTouchResult::Kind::OpenModule),
                    static_cast<int>(hit.kind));
  TEST_ASSERT_EQUAL_UINT32(0, hit.module_index);
}

void test_status_banner_does_not_open_an_app() {
  const HomeTouchResult hit =
      hit_home(kStatusX + 20, kStatusY + 10, 2, 0, true);
  TEST_ASSERT_EQUAL(static_cast<int>(HomeTouchResult::Kind::None),
                    static_cast<int>(hit.kind));
}

void test_name_row_stays_tappable_while_status_shows() {
  const HomeTouchResult hit =
      hit_home(kAppListX + 20, kAppListY + 8, 2, 0, true);
  TEST_ASSERT_EQUAL(static_cast<int>(HomeTouchResult::Kind::OpenModule),
                    static_cast<int>(hit.kind));
}

void test_scroll_hit_only_when_needed() {
  const HomeTouchResult none =
      hit_home(kChevronX + 4, kAppListY + 8, 2, 0, false);
  TEST_ASSERT_EQUAL(static_cast<int>(HomeTouchResult::Kind::None),
                    static_cast<int>(none.kind));

  const HomeTouchResult down =
      hit_home(kChevronX + 4, kAppListY + 112, 6, 0, false);
  TEST_ASSERT_EQUAL(static_cast<int>(HomeTouchResult::Kind::ScrollDown),
                    static_cast<int>(down.kind));
}

void test_left_pane_is_not_a_button() {
  const HomeTouchResult hit = hit_home(40, 80, 2, 0, false);
  TEST_ASSERT_EQUAL(static_cast<int>(HomeTouchResult::Kind::None),
                    static_cast<int>(hit.kind));
}

void test_pomodoro_start_and_pause() {
  PomodoroModel model;
  TEST_ASSERT_TRUE(model.start(1000));
  TEST_ASSERT_EQUAL(static_cast<int>(PomodoroStatus::Running),
                    static_cast<int>(model.snapshot().status));
  TEST_ASSERT_TRUE(model.pause(1500));
  TEST_ASSERT_EQUAL(static_cast<int>(PomodoroStatus::Paused),
                    static_cast<int>(model.snapshot().status));
}

void test_completed_focus_counts_as_character_xp() {
  PomodoroPreset preset;
  preset.focus_minutes = 1;
  preset.break_minutes = 1;
  preset.cycles = 2;
  PomodoroModel model;
  TEST_ASSERT_TRUE(model.configure(preset));
  TEST_ASSERT_TRUE(model.start(0));
  TEST_ASSERT_TRUE(model.tick(60U * 1000U));
  TEST_ASSERT_EQUAL_UINT32(1, model.snapshot().completed_sessions);
  TEST_ASSERT_EQUAL(static_cast<int>(PomodoroStatus::AwaitingTransition),
                    static_cast<int>(model.snapshot().status));
  TEST_ASSERT_EQUAL(static_cast<int>(PomodoroSegment::Break),
                    static_cast<int>(model.snapshot().segment));
}

void test_restore_running_becomes_paused() {
  PomodoroModel model;
  TEST_ASSERT_TRUE(model.start(0));
  auto stored = model.snapshot();
  stored.status = PomodoroStatus::Running;
  PomodoroModel restored;
  TEST_ASSERT_TRUE(restored.restore(stored));
  TEST_ASSERT_EQUAL(static_cast<int>(PomodoroStatus::Paused),
                    static_cast<int>(restored.snapshot().status));
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_rank_starts_at_one);
  RUN_TEST(test_rank_advances_every_four_sessions);
  RUN_TEST(test_home_scroll_clamps);
  RUN_TEST(test_name_row_opens_visible_module);
  RUN_TEST(test_status_banner_does_not_open_an_app);
  RUN_TEST(test_name_row_stays_tappable_while_status_shows);
  RUN_TEST(test_scroll_hit_only_when_needed);
  RUN_TEST(test_left_pane_is_not_a_button);
  RUN_TEST(test_pomodoro_start_and_pause);
  RUN_TEST(test_completed_focus_counts_as_character_xp);
  RUN_TEST(test_restore_running_becomes_paused);
  return UNITY_END();
}
