Feature: /emoji panel (REQ-EMOJI-2 to REQ-EMOJI-5)
  Shared by macOS and linows; keys are Enter and Shift+Enter.

  Scenario: Open the panel
    When the user opens /emoji
    Then the search field is focused and recents are shown

  Scenario: Enter copies
    Given the query "smile" shows results
    When the user presses Enter on the selected emoji
    Then the glyph is on the clipboard and a "Copied" notice is shown

  Scenario: Shift+Enter inserts
    Given the query "smile" shows results and insertion is permitted
    When the user presses Shift+Enter on the selected emoji
    Then the glyph is typed into the previously focused app

  Scenario: Insertion denied falls back to copy
    Given insertion is not permitted
    When the user presses Shift+Enter on the selected emoji
    Then the glyph is on the clipboard and a notice says it was copied instead

  Scenario: Recents order
    Given emoji A then B were used
    When the panel opens with an empty query
    Then B is listed before A

  Scenario: Recents de-duplicate
    Given A, B, then A were used
    Then recents are A, B with A first

  Scenario: Recents are capped at 20
    Given 21 distinct emoji were used
    Then 20 recents remain and the oldest is dropped

  Scenario: Recents survive restart
    Given emoji A was used
    When the app restarts and /emoji is opened
    Then A is still in recents

  Scenario: No match
    When the query "zzzzqx" is searched
    Then "No emoji found" is shown and Enter does nothing
