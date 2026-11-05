Feature: Emoji search (REQ-EMOJI-1, REQ-EMOJI-6, NFR-EMOJI-1)

  Scenario: Match by name
    When the query "smile" is searched
    Then the hits include emoji whose name or keywords contain "smile"

  Scenario: Match by keyword
    When a keyword that is not part of an emoji's name is searched
    Then emoji carrying that keyword are returned

  Scenario: Case and whitespace are ignored
    When the query "  SMILE " is searched
    Then the hits equal those for "smile"

  Scenario: Name matches rank above keyword matches
    When a query matches one emoji by name prefix and another only by keyword
    Then the name-prefix match is listed first

  Scenario: Limit is honoured
    When the query "face" is searched with a limit of 5
    Then at most 5 hits are returned

  Scenario: Empty query
    When the query "" is searched
    Then no hits are returned

  Scenario: No match
    When a query matching nothing is searched
    Then no hits are returned and no error occurs

  Scenario: Lookup of a persisted glyph
    When a known glyph is looked up
    Then its name is returned
    And an unknown glyph returns nothing

  Scenario: Latency budget
    When a query is run against the full dataset
    Then results return within 20 ms
    And no network request is made
