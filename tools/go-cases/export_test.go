package cases

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"testing"
	"unicode"
	"unicode/utf8"

	"golang.org/x/text/language"
)

func rustCaseFlags(scalar rune) uint8 {
	value, _ := trie.lookup([]byte(string(scalar)))
	properties := info(value)
	var flags uint8
	for index, enabled := range []bool{properties.isCased(), properties.isCaseIgnorable(), properties.isBreak(), properties.isMid(), unicode.IsUpper(scalar)} {
		if enabled {
			flags |= 1 << index
		}
	}
	return flags
}

func rustCaseMapping(test *testing.T, scalar rune, mapper mapFunc) string {
	state := context{src: []byte(string(scalar)), dst: make([]byte, 64), atEOF: true}
	if !state.next() || !mapper(&state) || state.err != nil {
		test.Fatalf("Unable to map U+%04X: %v", scalar, state.err)
	}
	return string(state.dst[:state.pDst])
}

func rustCaseLiteral(value string) string {
	var output strings.Builder
	output.WriteByte('"')
	for _, scalar := range value {
		fmt.Fprintf(&output, `\u{%x}`, scalar)
	}
	output.WriteByte('"')
	return output.String()
}

func TestRustExportCaseTables(test *testing.T) {
	if UnicodeVersion != "17.0.0" || unicode.Version != "17.0.0" {
		test.Fatal("Casing requires Unicode 17.0.0")
	}
	type expectation struct {
		Input string `json:"input"`
		Output string `json:"output"`
	}
	var expected []expectation
	seen := make(map[string]bool)
	titler := Title(language.English)
	add := func(input string) {
		if !seen[input] {
			seen[input] = true
			expected = append(expected, expectation{input, titler.String(input)})
		}
	}
	var ranges, mappings strings.Builder
	ranges.WriteString("pub const CASE_RANGES: &[(u32, u32, u8)] = &[\n")
	mappings.WriteString("pub const CASE_MAPPINGS: &[(u32, &str, &str, char)] = &[\n")
	start := rune(0)
	previous := rustCaseFlags(0)
	representatives := make(map[uint8]rune)
	var representativeOrder []rune
	mappingCount := 0
	for scalar := rune(0); scalar <= unicode.MaxRune; scalar++ {
		flags := rustCaseFlags(scalar)
		changed := flags != previous
		if changed {
			fmt.Fprintf(&ranges, "    (0x%x, 0x%x, %d),\n", start, scalar-1, previous)
			start, previous = scalar, flags
		}
		if !utf8.ValidRune(scalar) {
			continue
		}
		if _, present := representatives[flags]; !present {
			representatives[flags] = scalar
			representativeOrder = append(representativeOrder, scalar)
		}
		original := string(scalar)
		titled := rustCaseMapping(test, scalar, title)
		lowered := rustCaseMapping(test, scalar, lower)
		simpleLower := unicode.ToLower(scalar)
		mapped := titled != original || lowered != original || simpleLower != scalar
		if mapped {
			fmt.Fprintf(&mappings, "    (0x%x, %s, %s, '\\u{%x}'),\n", scalar, rustCaseLiteral(titled), rustCaseLiteral(lowered), simpleLower)
			mappingCount++
		}
		if mapped || changed {
			for _, input := range []string{original, original + "ABC", "a" + original + "BC", "a\u03a3" + original + "\u03a3", "ab'" + original + "'CD"} {
				add(input)
			}
		}
	}
	fmt.Fprintf(&ranges, "    (0x%x, 0x%x, %d),\n];\n", start, unicode.MaxRune, previous)
	mappings.WriteString("];\n")
	for _, first := range representativeOrder {
		for _, second := range representativeOrder {
			add("a" + string(first) + string(second) + "B")
			add("a\u03a3" + string(first) + string(second) + "\u03a3")
		}
	}
	for _, count := range []int{0, 1, 29, 30, 31, 32, 60, 61, 62} {
		for _, ignorable := range []string{"'", ".", "\u0301", "\u0345", "\u00ad", "\u200d"} {
			add("a\u03a3" + strings.Repeat(ignorable, count) + "\u03a3ABC")
		}
	}
	if err := os.WriteFile(os.Getenv("RUST_REFERENCE_CASES"), []byte(ranges.String()+mappings.String()), 0644); err != nil {
		test.Fatal(err)
	}
	data, err := json.Marshal(expected)
	if err != nil {
		test.Fatal(err)
	}
	if err := os.WriteFile(os.Getenv("RUST_REFERENCE_CASE_FIXTURE"), data, 0644); err != nil {
		test.Fatal(err)
	}
	test.Logf("Exported %d mappings across all Unicode scalars and %d contextual title cases", mappingCount, len(expected))
}