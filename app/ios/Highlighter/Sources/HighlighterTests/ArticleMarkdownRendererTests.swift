import Testing
import UIKit
@testable import Highlighter

struct ArticleMarkdownRendererTests {
    @Test func rendersTextAfterReportedBorrowedMapsBreakPoint() {
        let content = """
        When you ask Google for directions, the request leaving your phone carries a timestamped record that pairs your current coordinates with your intended coordinates, attached to an account that already knows your phone number, your home, your work, your contacts, and the pattern of your previous trips. The route itself is the smaller half of the leak. The richer half is the intent: you have just told a third party where you will be and roughly when. Last Tuesday's trip to the clinic, tonight's drive to a lawyer's office, every trip you would prefer to keep out of a permanent index of your life: each one sits in a row of a database you do not own and cannot delete. Multiply that by every trip you take, store it next to every other person's trips for the rest of your life, and you have a panopticon of physical movement that the surveillance states of the twentieth century could only have dreamed of funding.

        The economics here are blunt. Google gives away maps as a loss leader for a behavioral product.
        """

        let output = MarkdownRenderer.render(
            content: content,
            highlights: [],
            accent: .systemOrange,
            tint: .systemOrange,
            ink: .label,
            muted: .secondaryLabel,
            highlightContent: { _ in fatalError("No highlights are rendered in this test") }
        )

        let rendered = output.segments.reduce(into: "") { text, segment in
            if case .text(let attributed) = segment {
                text.append(attributed.string)
            }
        }

        #expect(rendered.contains("dreamed of funding."))
        #expect(rendered.contains("The economics here are blunt."))
    }
}
