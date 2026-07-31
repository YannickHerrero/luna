import Testing
@testable import Luna

struct AgentControlsTests {
    @Test
    func keepsOrFallsBackToASupportedThinkingLevel() {
        #expect(
            preferredThinkingLevel(current: .medium, supported: [.off, .medium, .high])
                == .medium
        )
        #expect(preferredThinkingLevel(current: .max, supported: [.off, .high]) == .high)
        #expect(preferredThinkingLevel(current: .high, supported: [.minimal, .low]) == .low)
        #expect(preferredThinkingLevel(current: .high, supported: []) == .off)
    }

    @Test
    func calculatesAndClampsContextPercentages() {
        #expect(agentContextPercent(tokens: 48_000, contextWindow: 200_000, fallback: nil) == 24)
        #expect(agentContextPercent(tokens: 300_000, contextWindow: 200_000, fallback: nil) == 100)
        #expect(agentContextPercent(tokens: nil, contextWindow: 200_000, fallback: 18.4) == 18.4)
        #expect(agentContextPercent(tokens: nil, contextWindow: nil, fallback: 120) == 100)
        #expect(agentContextPercent(tokens: nil, contextWindow: nil, fallback: nil) == nil)
    }

    @Test
    func formatsContextTokenCountsCompactly() {
        #expect(formatAgentTokens(999) == "999")
        #expect(formatAgentTokens(1_500) == "1.5K")
        #expect(formatAgentTokens(48_000) == "48K")
        #expect(formatAgentTokens(200_000) == "200K")
        #expect(formatAgentTokens(1_250_000) == "1.3M")
    }
}
