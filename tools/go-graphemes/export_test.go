package uniseg

import (
	"fmt"
	"os"
	"strings"
	"testing"
	"unicode"
)

func TestRustExportGraphemeTables(test *testing.T) {
	var ranges strings.Builder
	ranges.WriteString("pub const GRAPHEME_RANGES: &[(u32, u32, usize)] = &[\n")
	indices := make(map[int]int)
	var representatives []rune
	start := rune(0)
	previous := -1
	for scalar := rune(0); scalar <= unicode.MaxRune; scalar++ {
		property := propertyGraphemes(scalar)
		index, exists := indices[property]
		if !exists {
			index = len(representatives)
			indices[property] = index
			representatives = append(representatives, scalar)
		}
		if index != previous {
			if previous >= 0 {
				fmt.Fprintf(&ranges, "    (0x%x, 0x%x, %d),\n", start, scalar-1, previous)
			}
			start, previous = scalar, index
		}
	}
	fmt.Fprintf(&ranges, "    (0x%x, 0x%x, %d),\n];\n", start, unicode.MaxRune, previous)
	ranges.WriteString("pub const GRAPHEME_TRANSITIONS: &[&[(usize, bool)]] = &[\n")
	for state := -1; state <= grRIEven; state++ {
		ranges.WriteString("    &[\n")
		for _, representative := range representatives {
			next, _, boundary := transitionGraphemeState(state, representative)
			fmt.Fprintf(&ranges, "        (%d, %t),\n", next+1, boundary)
		}
		ranges.WriteString("    ],\n")
	}
	ranges.WriteString("];\n")
	if err := os.WriteFile(os.Getenv("RUST_REFERENCE_GRAPHEMES"), []byte(ranges.String()), 0644); err != nil {
		test.Fatal(err)
	}
	test.Logf("Exported %d grapheme classes and %d transition states across all Unicode scalars", len(representatives), grRIEven+2)
}