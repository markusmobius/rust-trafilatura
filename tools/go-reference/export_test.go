package trafilatura

import (
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	nurl "net/url"
	"os"
	"path/filepath"
	"regexp"
	"regexp/syntax"
	"runtime"
	"runtime/debug"
	"sort"
	"strconv"
	"strings"
	"testing"
	"time"
	"unicode"
	"unicode/utf8"

	"github.com/andybalholm/cascadia"
	"github.com/forPelevin/gomoji"
	"github.com/go-shiori/dom"
	"github.com/markusmobius/go-htmldate"
	"github.com/markusmobius/go-trafilatura/v2/internal/etree"
	"github.com/markusmobius/go-trafilatura/v2/internal/lru"
	"github.com/markusmobius/go-trafilatura/v2/internal/selector"
	"golang.org/x/net/html"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

const rustReferenceCommit = "72dce36bfe95502563533cf68a9050370a3d7081"

type rustTextCase struct {
	Input    string `json:"input"`
	Trimmed  string `json:"trimmed"`
	Words    int    `json:"words"`
	Filtered string `json:"filtered"`
	Image    bool   `json:"image"`
}

type rustListCase struct {
	Input   []string `json:"input"`
	Output  []string `json:"output"`
	Cleaned []string `json:"cleaned"`
}

type rustElementSnapshot struct {
	Tag         string   `json:"tag"`
	ID          string   `json:"id"`
	Text        string   `json:"text"`
	Tail        string   `json:"tail"`
	IterText    string   `json:"iter_text"`
	HTML        string   `json:"html"`
	Image       bool     `json:"image"`
	Descendants []string `json:"descendants"`
	Selected    []string `json:"selected"`
}

type rustDOMCase struct {
	HTML      string                `json:"html"`
	Operation string                `json:"operation"`
	Value     string                `json:"value"`
	Output    string                `json:"output"`
	Elements  []rustElementSnapshot `json:"elements"`
}

type rustReferenceFixture struct {
	Commit    string             `json:"commit"`
	GoVersion string             `json:"go_version"`
	Unicode   string             `json:"unicode"`
	Modules   map[string]string  `json:"modules"`
	Config    *Config            `json:"config"`
	Text      []rustTextCase     `json:"text"`
	Lists     []rustListCase     `json:"lists"`
	DOM       []rustDOMCase      `json:"dom"`
	Metadata  []rustMetadataCase `json:"metadata"`
	Dates     []rustDateCase     `json:"dates"`
	URLs      []rustURLCase      `json:"urls"`
	URLResolution []rustURLResolutionCase `json:"url_resolution"`
	Normalization []rustNormalizationCase `json:"normalization"`
	Casing []rustStringCase `json:"casing"`
	Emojis []rustStringCase `json:"emojis"`
	Preparation []rustPreparationCase `json:"preparation"`
	Conversion []rustConversionCase `json:"conversion"`
	LinkDensity []rustLinkDensityCase `json:"link_density"`
	TextNodes []rustTextNodeCase `json:"text_nodes"`
	Handlers []rustHandlerCase `json:"handlers"`
	Spans []rustSpanCase `json:"spans"`
	Selectors []rustSelectorCase `json:"selectors"`
	Pruning []rustPruningCase `json:"pruning"`
	Content []rustContentCase `json:"content"`
	Baseline []rustBaselineCase `json:"baseline"`
	BaselineText []rustStringCase `json:"baseline_text"`
	PostCleaning []rustStringCase `json:"post_cleaning"`
	FallbackSelection []rustFallbackSelectionCase `json:"fallback_selection"`
	Sanitization []rustSanitizationCase `json:"sanitization"`
	NativeFallbacks []rustNativeFallbackCase `json:"native_fallbacks"`
	LanguageGates []rustLanguageGateCase `json:"language_gates"`
	Languages []rustLanguageCase `json:"languages"`
	Forums []rustForumCase `json:"forums"`
	Sequences []rustContentCase `json:"sequences"`
	CSS []rustCSSCase `json:"css"`
	CSSRegex []rustRegexCase `json:"css_regex"`
	Extraction []rustExtractionCase `json:"extraction"`
	ParsedInputs map[string]any `json:"parsed_inputs"`
}

func rustParsedInput(node *html.Node) any {
	if node.Type == html.DocumentNode { return rustParsedInput(dom.QuerySelector(node, "html")) }
	if node.Type == html.CommentNode { return map[string]any{"comment": node.Data} }
	if node.Type == html.TextNode { return node.Data }
	attributes := [][2]string{}
	for _, attribute := range node.Attr {
		key := attribute.Key
		if attribute.Namespace != "" { key = attribute.Namespace + ":" + key }
		attributes = append(attributes, [2]string{key, attribute.Val})
	}
	children := []any{}
	for child := node.FirstChild; child != nil; child = child.NextSibling {
		if child.Type == html.ElementNode || child.Type == html.CommentNode || child.Type == html.TextNode {
			children = append(children, rustParsedInput(child))
		}
	}
	return map[string]any{"tag": node.Data, "attributes": attributes, "children": children}
}

type rustExtractionCase struct {
	HTML string `json:"html"`
	Focus ExtractionFocus `json:"focus"`
	Flags int `json:"flags"`
	Variant int `json:"variant"`
	Error string `json:"error"`
	Content string `json:"content"`
	Comments string `json:"comments"`
	ContentText string `json:"content_text"`
	CommentsText string `json:"comments_text"`
	Metadata Metadata `json:"metadata"`
}

type rustRegexCase struct {
	Pattern string `json:"pattern"`
	Input string `json:"input"`
	Valid bool `json:"valid"`
	Matches bool `json:"matches"`
}

type rustCSSCase struct {
	HTML string `json:"html"`
	Selector string `json:"selector"`
	Valid bool `json:"valid"`
	Matches []string `json:"matches"`
	Pruned string `json:"pruned"`
}

type rustLanguageGateCase struct {
	HTML string `json:"html"`
	Target string `json:"target"`
	Strict bool `json:"strict"`
	Accepted bool `json:"accepted"`
}

type rustLanguageCase struct {
	Content string `json:"content"`
	Comments string `json:"comments"`
	Language string `json:"language"`
}

type rustForumCase struct {
	HTML string `json:"html"`
	Forum bool `json:"forum"`
}

type rustNativeFallbackCase struct {
	HTML string `json:"html"`
	Extracted string `json:"extracted"`
	Focus ExtractionFocus `json:"focus"`
	Flags int `json:"flags"`
	Variant int `json:"variant"`
	Output string `json:"output"`
	Text string `json:"text"`
	Rescued string `json:"rescued"`
	RescuedText string `json:"rescued_text"`
}

type rustFallbackSelectionCase struct {
	Candidate string `json:"candidate"`
	Extracted string `json:"extracted"`
	Focus ExtractionFocus `json:"focus"`
	Minimum int `json:"minimum"`
	CandidateLength int `json:"candidate_length"`
	ExtractedLength int `json:"extracted_length"`
	Usable bool `json:"usable"`
}

type rustSanitizationCase struct {
	HTML string `json:"html"`
	Focus ExtractionFocus `json:"focus"`
	Flags int `json:"flags"`
	Output string `json:"output"`
}

type rustBaselineCase struct {
	HTML string `json:"html"`
	Bodies []string `json:"bodies"`
	Teasers []string `json:"teasers"`
	Output string `json:"output"`
	Text string `json:"text"`
	Plain string `json:"plain"`
	Flat string `json:"flat"`
	Cleaned string `json:"cleaned"`
}

type rustContentCase struct {
	HTML string `json:"html"`
	Focus ExtractionFocus `json:"focus"`
	Flags int `json:"flags"`
	Content string `json:"content"`
	ContentText string `json:"content_text"`
	Comments string `json:"comments"`
	CommentsText string `json:"comments_text"`
	Mutated string `json:"mutated"`
}

type rustSelectorCase struct {
	Tag string `json:"tag"`
	Value string `json:"value"`
	Layout int `json:"layout"`
	Matches []bool `json:"matches"`
}

type rustPruningCase struct {
	HTML string `json:"html"`
	Group int `json:"group"`
	Backup bool `json:"backup"`
	Output string `json:"output"`
	Mutated string `json:"mutated"`
}

type rustSpanCase struct {
	Input string `json:"input"`
	Output int `json:"output"`
}

type rustHandlerCase struct {
	HTML string `json:"html"`
	Handler string `json:"handler"`
	Focus ExtractionFocus `json:"focus"`
	Images bool `json:"images"`
	Links bool `json:"links"`
	Deduplicate bool `json:"deduplicate"`
	Output string `json:"output"`
	Mutated string `json:"mutated"`
}

type rustTextNodeCase struct {
	HTML string `json:"html"`
	Fix bool `json:"fix"`
	Spaces bool `json:"spaces"`
	Light bool `json:"light"`
	Capacity int `json:"capacity"`
	Accepted []bool `json:"accepted"`
	Filtered bool `json:"filtered"`
	Output string `json:"output"`
}

type rustLinkDensityCase struct {
	HTML string `json:"html"`
	Focus ExtractionFocus `json:"focus"`
	IncludeImages bool `json:"include_images"`
	Nodes []rustLinkDensityNode `json:"nodes"`
	Deleted []string `json:"deleted"`
}

type rustLinkDensityNode struct {
	Length int `json:"length"`
	Short int `json:"short"`
	NonEmpty int `json:"non_empty"`
	Selected int `json:"selected"`
	High bool `json:"high"`
	TableHigh bool `json:"table_high"`
}

type rustConversionCase struct {
	HTML string `json:"html"`
	ExcludeTables bool `json:"exclude_tables"`
	IncludeImages bool `json:"include_images"`
	IncludeLinks bool `json:"include_links"`
	OriginalURL string `json:"original_url"`
	Converted string `json:"converted"`
}

type rustPreparationCase struct {
	HTML string `json:"html"`
	Focus ExtractionFocus `json:"focus"`
	ExcludeTables bool `json:"exclude_tables"`
	IncludeImages bool `json:"include_images"`
	Cleaned string `json:"cleaned"`
}

type rustStringCase struct {
	Input string `json:"input"`
	Output string `json:"output"`
}

type rustMetadataCase struct {
	HTML       string   `json:"html"`
	Title      string   `json:"title"`
	TitleParts []string `json:"title_parts"`
	Sitename   string   `json:"sitename"`
	License    string   `json:"license"`
	DomURL     string   `json:"dom_url"`
	OpenGraph Metadata `json:"open_graph"`
	Meta Metadata `json:"meta"`
	JSONLD Metadata `json:"json_ld"`
	Extracted Metadata `json:"extracted"`
	DomAuthor string `json:"dom_author"`
	Categories []string `json:"categories"`
	Tags []string `json:"tags"`
}

type rustNormalizationCase struct {
	Input string `json:"input"`
	Unescaped string `json:"unescaped"`
	Cleaned string `json:"cleaned"`
	JSON string `json:"json"`
	Tags string `json:"tags"`
	Name string `json:"name"`
	Authors string `json:"authors"`
	AuthorsExisting string `json:"authors_existing"`
	Title string `json:"title"`
	NoEmoji string `json:"no_emoji"`
}

type rustURLResolutionCase struct {
	Input string `json:"input"`
	Base string `json:"base"`
	Created string `json:"created"`
	Validated string `json:"validated"`
	Absolute bool `json:"absolute"`
}

type rustURLCase struct {
	Input             string `json:"input"`
	Absolute          bool   `json:"absolute"`
	Base              string `json:"base"`
	Domain            string `json:"domain"`
	Validated         string `json:"validated"`
	ValidatedAbsolute bool   `json:"validated_absolute"`
}

type rustDateCase struct {
	HTML     string `json:"html"`
	Mode     int    `json:"mode"`
	Fallback bool   `json:"fallback"`
	Custom   int    `json:"custom"`
	Override int    `json:"override"`
	URL      string `json:"url"`
	Date     string `json:"date"`
}

func rustExportEmojis(test *testing.T, tables *strings.Builder, result *rustReferenceFixture) {
	source, err := parser.ParseFile(token.NewFileSet(), os.Getenv("RUST_REFERENCE_EMOJI_SOURCE"), nil, 0)
	if err != nil {
		test.Fatal(err)
	}
	var emojis []string
	ast.Inspect(source, func(node ast.Node) bool {
		declaration, ok := node.(*ast.ValueSpec)
		if !ok || len(declaration.Names) != 1 || declaration.Names[0].Name != "emojiMap" {
			return true
		}
		mapping := declaration.Values[0].(*ast.CompositeLit)
		for _, entry := range mapping.Elts {
			literal := entry.(*ast.KeyValueExpr).Key.(*ast.BasicLit)
			key, err := strconv.Unquote(literal.Value)
			if err != nil {
				test.Fatal(err)
			}
			emojis = append(emojis, key)
		}
		return false
	})
	if len(emojis) != len(gomoji.AllEmojis()) {
		test.Fatal("Emoji dictionary key count mismatch")
	}
	sort.Strings(emojis)
	tables.WriteString("pub const EMOJIS: &[&str] = &[\n")
	var singles []rune
	for _, emoji := range emojis {
		tables.WriteString("    \"")
		for _, scalar := range emoji {
			fmt.Fprintf(tables, `\u{%x}`, scalar)
		}
		tables.WriteString("\",\n")
		if utf8.RuneCountInString(emoji) == 1 {
			scalar, _ := utf8.DecodeRuneInString(emoji)
			singles = append(singles, scalar)
		}
		for _, input := range []string{emoji, "a" + emoji + "b", "\u0301" + emoji + "\u0301", "\u0600" + emoji, emoji + "\u200dA", "\U0001f469\u200d" + emoji, emoji + "\ufe0e", "\x00 " + emoji + " \x00"} {
			result.Emojis = append(result.Emojis, rustStringCase{input, gomoji.RemoveEmojis(input)})
		}
	}
	tables.WriteString("];\npub const EMOJI_SCALARS: &[u32] = &[\n")
	for _, scalar := range singles {
		fmt.Fprintf(tables, "    0x%x,\n", scalar)
	}
	tables.WriteString("];\npub const EMOJI_TRIM_RANGES: &[(u32, u32)] = &[\n")
	start := rune(-1)
	for scalar := rune(0); scalar <= unicode.MaxRune+1; scalar++ {
		keep := scalar <= unicode.MaxRune && unicode.IsGraphic(scalar) && unicode.IsPrint(scalar) && !unicode.In(scalar, unicode.Variation_Selector)
		if keep && start < 0 {
			start = scalar
		}
		if !keep && start >= 0 {
			fmt.Fprintf(tables, "    (0x%x, 0x%x),\n", start, scalar-1)
			start = -1
		}
	}
	tables.WriteString("];\n")
}

func rustExportCharacterClass(tables *strings.Builder, name string, keep func(rune) bool) {
	fmt.Fprintf(tables, "pub const %s: &str = r\"", name)
	start := rune(-1)
	for scalar := rune(0); scalar <= unicode.MaxRune+1; scalar++ {
		included := scalar <= unicode.MaxRune && keep(scalar)
		if included && start < 0 {
			start = scalar
		}
		if !included && start >= 0 {
			fmt.Fprintf(tables, `\u{%x}-\u{%x}`, start, scalar-1)
			start = -1
		}
	}
	tables.WriteString("\";\n")
}

func TestRustExportReference(test *testing.T) {
	if runtime.Version() != "go1.27.1" {
		test.Fatal("Go reference requires go1.27.1")
	}
	time.Local = time.UTC
	graph, err := os.ReadFile(os.Getenv("RUST_REFERENCE_MODULES"))
	if err != nil {
		test.Fatal(err)
	}
	result := rustReferenceFixture{Commit: rustReferenceCommit, GoVersion: runtime.Version(), Unicode: unicode.Version, Config: DefaultConfig()}
	if err := json.Unmarshal(graph, &result.Modules); err != nil {
		test.Fatal(err)
	}
	caseFixture, err := os.ReadFile(os.Getenv("RUST_REFERENCE_CASE_FIXTURE"))
	if err != nil {
		test.Fatal(err)
	}
	if err := json.Unmarshal(caseFixture, &result.Casing); err != nil {
		test.Fatal(err)
	}
	info, ok := debug.ReadBuildInfo()
	if !ok {
		test.Fatal("Go build information unavailable")
	}
	for _, dependency := range info.Deps {
		if dependency.Replace != nil || result.Modules[dependency.Path] != dependency.Version {
			test.Fatalf("Unexpected linked module: %s %s", dependency.Path, dependency.Version)
		}
	}
	inputs := []string{"", " one\ttwo\nthree ", "a\x00b\vc\fd\u200be\u00adf", "a\u00a0b\u2003c", "photo.JPG", "./image.heic?size=2", "file.png_no", "\u5b57.png\u5b57", ".png", "../.png", strings.Repeat("a", 8188) + ".png", strings.Repeat("a", 8189) + ".png"}
	var tables strings.Builder
	tables.WriteString("pub const KEEP_RANGES: &[(u32, u32)] = &[\n")
	start := rune(-1)
	for scalar := rune(0); scalar <= unicode.MaxRune+1; scalar++ {
		keep := scalar <= unicode.MaxRune && (unicode.IsPrint(scalar) || unicode.IsSpace(scalar))
		if scalar <= unicode.MaxRune && unicode.IsSpace(scalar) {
			inputs = append(inputs, "a"+string(scalar)+"b")
		}
		if keep && start < 0 {
			start = scalar
		}
		if !keep && start >= 0 {
			fmt.Fprintf(&tables, "    (0x%x, 0x%x),\n", start, scalar-1)
			for _, boundary := range []rune{start - 1, start, scalar - 1, scalar} {
				if boundary >= 0 && boundary <= unicode.MaxRune && (boundary < 0xd800 || boundary > 0xdfff) {
					inputs = append(inputs, "a"+string(boundary)+"b")
				}
			}
			start = -1
		}
	}
	tables.WriteString("];\n")
	rustExportEmojis(test, &tables, &result)
	rustExportCharacterClass(&tables, "NUMBER_CLASS", unicode.IsNumber)
	tables.WriteString("pub const DIGIT_RANGES: &[(u32, u32, u32)] = &[\n")
	for _, value := range unicode.Digit.R16 { fmt.Fprintf(&tables, "    (0x%x, 0x%x, %d),\n", value.Lo, value.Hi, value.Stride) }
	for _, value := range unicode.Digit.R32 { fmt.Fprintf(&tables, "    (0x%x, 0x%x, %d),\n", value.Lo, value.Hi, value.Stride) }
	tables.WriteString("];\n")
	tables.WriteString("pub const SIMPLE_FOLD: &[(u32, u32)] = &[\n")
	for scalar := rune(0); scalar <= unicode.MaxRune; scalar++ {
		if folded := unicode.SimpleFold(scalar); folded != scalar { fmt.Fprintf(&tables, "    (0x%x, 0x%x),\n", scalar, folded) }
	}
	tables.WriteString("];\n")
	var regexNames []string
	for name := range unicode.Categories { regexNames = append(regexNames, name) }
	for name := range unicode.Scripts { regexNames = append(regexNames, name) }
	regexNames = append(regexNames, "Any", "ASCII", "Assigned")
	canonicalRegexName := func(name string) string { return strings.ToLower(strings.NewReplacer("_", "", "-", "", " ", "").Replace(name)) }
	sort.Slice(regexNames, func(first, second int) bool { return canonicalRegexName(regexNames[first]) < canonicalRegexName(regexNames[second]) })
	tables.WriteString("pub const REGEX_UNICODE_GROUPS: &[(&str, &[(u32, u32)], &[(u32, u32)])] = &[\n")
	for _, name := range regexNames {
		fmt.Fprintf(&tables, "    (%q, ", canonicalRegexName(name))
		for _, flags := range []syntax.Flags{syntax.Perl, syntax.Perl | syntax.FoldCase} {
			expression, err := syntax.Parse(`\p{`+name+`}`, flags)
			if err != nil { test.Fatal(err) }
			var ranges []rune
			switch expression.Op {
			case syntax.OpCharClass: ranges = expression.Rune
			case syntax.OpAnyChar: ranges = []rune{0, unicode.MaxRune}
			case syntax.OpAnyCharNotNL: ranges = []rune{0, '\n'-1, '\n'+1, unicode.MaxRune}
			case syntax.OpLiteral:
				if len(expression.Rune) != 1 { test.Fatal("unexpected Unicode literal") }
				for scalar := expression.Rune[0]; ; { ranges = append(ranges, scalar, scalar); if expression.Flags&syntax.FoldCase == 0 { break }; scalar = unicode.SimpleFold(scalar); if scalar == expression.Rune[0] { break } }
			default: test.Fatal("unexpected Unicode class", expression)
			}
			tables.WriteString("&[")
			for index := 0; index < len(ranges); index += 2 { fmt.Fprintf(&tables, "(0x%x, 0x%x),", ranges[index], ranges[index+1]) }
			tables.WriteString("], ")
		}
		tables.WriteString("),\n")
	}
	tables.WriteString("];\n")
	regexAliases := make(map[string]string)
	for name, actual := range unicode.CategoryAliases { regexAliases[canonicalRegexName(name)] = canonicalRegexName(actual) }
	var aliasNames []string
	for name := range regexAliases { aliasNames = append(aliasNames, name) }
	sort.Strings(aliasNames)
	tables.WriteString("pub const REGEX_UNICODE_ALIASES: &[(&str, &str)] = &[\n")
	for _, name := range aliasNames { fmt.Fprintf(&tables, "    (%q, %q),\n", name, regexAliases[name]) }
	tables.WriteString("];\n")
	rustExportCharacterClass(&tables, "AUTHOR_WORD_CLASS", func(scalar rune) bool {
		return unicode.IsLetter(scalar) || unicode.IsMark(scalar) || unicode.IsNumber(scalar) || scalar == '_'
	})
	if err := os.WriteFile(os.Getenv("RUST_REFERENCE_UNICODE"), []byte(tables.String()), 0644); err != nil {
		test.Fatal(err)
	}
	for scalar := rune(0); scalar < 256; scalar++ {
		inputs = append(inputs, string(scalar), "a"+string(scalar)+"b")
	}
	for _, input := range inputs {
		result.Text = append(result.Text, rustTextCase{input, trim(input), strWordCount(input), removeControlCharacters(input), isImageFile(input)})
	}
	for _, input := range [][]string{nil, {""}, {"a,b", "b,c"}, {"a;b;c", "a,c;d"}, {"' one ', \"two\"", "one", "two"}, {" A ,a,A", "\u5b57; \u5b57;test"}, {"a; b, c", "x; y;z,q"}} {
		result.Lists = append(result.Lists, rustListCase{input, uniquifyLists(input...), cleanCatTags(input)})
	}
	for _, source := range []string{
		`<div id="target">before<!--one-->middle<span>A<em>B</em>C</span>after</div>tail<!--two-->end<p>next</p>`,
		`<div><span id="target">one<span>two</span>three</span>tail<br>last</div>`,
		`<p id="target"> a <b>b<i>c</i></b> d </p> <div>end</div>`,
		`<img id="target" src="photo.JPG">tail<!--comment-->more`,
		`<br id="target">tail`,
		`<table id="target"><tr><td>one<br>two<td>three</table>tail`,
		`<svg><a id="target" xlink:href="foreign.png" href="plain" xml:lang="fr">one<tspan>two</tspan></a>tail</svg>`,
		`<math><mtext id="target">one <b>two</b> three</mtext>tail</math>`,
		`<template id="target"><div>one<span>two</span></div></template>tail`,
		`<div id="target" data-src-other="lazy.webp">one &amp; two&nbsp;three</div>tail`,
		`<pre id="target">` + "\n\none\n<span>two</span>\n" + `</pre>tail`,
		`<div id="target"><script>if(a<b){x="&amp;"}</script><span>body</span></div>tail`,
	} {
		for _, operation := range []string{"inspect", "set_text", "clear_text", "set_tail", "clear_tail", "remove", "remove_keep_tail", "strip", "append", "strip_tags", "strip_elements", "strip_elements_keep_tail"} {
			input := "<html><head><title>Example</title></head><body>" + source + `<section id="destination">destination</section></body></html>`
			document, err := html.Parse(strings.NewReader(input))
			if err != nil {
				test.Fatal(err)
			}
			document = dom.Clone(document, true)
			target := dom.GetElementByID(document, "target")
			if target == nil {
				test.Fatal("missing target", input)
			}
			value := " replacement & \u5b57 "
			switch operation {
			case "set_text":
				etree.SetText(target, value)
			case "clear_text":
				etree.SetText(target, "")
			case "set_tail":
				etree.SetTail(target, value)
			case "clear_tail":
				etree.SetTail(target, "")
			case "remove":
				etree.Remove(target)
			case "remove_keep_tail":
				etree.Remove(target, true)
			case "strip":
				etree.Strip(target)
			case "append":
				etree.Append(dom.GetElementByID(document, "destination"), target)
			case "strip_tags":
				etree.StripTags(document, "span", "em")
			case "strip_elements":
				etree.StripElements(document, false, "span", "em")
			case "strip_elements_keep_tail":
				etree.StripElements(document, true, "span", "em")
			}
			current := rustDOMCase{HTML: input, Operation: operation, Value: value, Output: dom.OuterHTML(document)}
			for _, element := range etree.Iter(document) {
				descendants := []string{}
				selected := []string{}
				for _, descendant := range etree.IterDescendants(element) {
					descendants = append(descendants, dom.TagName(descendant))
				}
				for _, match := range etree.Iter(element, "div", "span", "br") {
					selected = append(selected, dom.TagName(match))
				}
				current.Elements = append(current.Elements, rustElementSnapshot{dom.TagName(element), dom.ID(element), etree.Text(element), etree.Tail(element), etree.IterText(element, "|"), etree.ToString(element), isImageElement(element), descendants, selected})
			}
			result.DOM = append(result.DOM, current)
		}
	}
	parsed, err := parser.ParseFile(token.NewFileSet(), "metadata_test.go", nil, 0)
	if err != nil {
		test.Fatal(err)
	}
	var metadataInputs []string
	normalizationInputs := append([]string(nil), inputs...)
	for _, declaration := range parsed.Decls {
		function, ok := declaration.(*ast.FuncDecl)
		if !ok || !strings.HasPrefix(function.Name.Name, "Test_Metadata_") {
			continue
		}
		ast.Inspect(function.Body, func(node ast.Node) bool {
			literal, ok := node.(*ast.BasicLit)
			if ok && literal.Kind == token.STRING {
				value, err := strconv.Unquote(literal.Value)
				if err == nil && strings.Contains(value, "<html") {
					metadataInputs = append(metadataInputs, value)
				}
				if err == nil && !strings.Contains(value, "<html") {
					normalizationInputs = append(normalizationInputs, value)
				}
			}
			return true
		})
	}
	for _, separator := range []string{"-", "|", ":", "\u2013", "\u2014", "\u2022", "\u00bb", "\u22c6", "\u2044"} {
		metadataInputs = append(metadataInputs, "<html><head><title>Example Title "+separator+" example.org</title></head></html>")
	}
	for _, length := range []int{0, 1, 2, 3, 199, 200, 201} {
		metadataInputs = append(metadataInputs, `<html><body><div class="entry-title">`+strings.Repeat("\u5b57", length)+`</div><h2>Fallback heading</h2></body></html>`)
	}
	for _, head := range []string{
		`<base href="https://base.example/"><link rel="canonical" href="https://canonical.example/a?x=1&amp;y=2">`,
		`<link rel="canonical" href=""><link rel="canonical" href="https://second.example/"><base href="https://base.example/">`,
		`<link rel="canonical"><link rel="alternate" hreflang="x-default" href="https://alternate.example/">`,
		`<link rel="canonical alternate" href="https://ignored.example/"><base href="https://base.example/">`,
		`<link rel="CANONICAL" href="https://ignored.example/"><base href="https://base.example/">`,
		`<link rel="canonical" href=" /article "><meta property="og:image" content="https://cdn.example/image.png">`,
		`<link rel="canonical" href="//other.example/article"><meta name="twitter:image" content="https://cdn.example/image.png">`,
		`<link rel="canonical" href="article"><meta property="og:url" content="https://base.example/root/">`,
		`<link rel="canonical" href="/article"><meta name="description" property="og:url" content="https://ignored.example/"><meta property="og:image" content="https://cdn.example/image.png">`,
		`<link rel="canonical" href="/article"><meta property="OG:URL" content="https://ignored.example/">`,
		`<link rel="canonical" href="/article"><meta property="og:url" content="not a URL"><meta property="og:image" content="https://cdn.example/image.png">`,
		`<link rel="canonical" href="https://example.org/%zz"><base href="https://base.example/">`,
		`<link rel="alternate" hreflang="en" href="https://ignored.example/"><link rel="alternate" hreflang="x-default" href="https://alternate.example/">`,
		`<base href="/root"><meta property="og:url" content="https://base.example/a">`,
	} {
		metadataInputs = append(metadataInputs, "<html><head>"+head+`</head><body><link rel="canonical" href="https://body.example/"></body></html>`)
	}
	for _, input := range metadataInputs {
		document, err := html.Parse(strings.NewReader(input))
		if err != nil {
			test.Fatal(err)
		}
		document = dom.Clone(document, true)
		title, first, second := examineTitleElement(document)
		result.Metadata = append(result.Metadata, rustMetadataCase{
			HTML: input, Title: extractDomTitle(document), TitleParts: []string{title, first, second},
			Sitename: extractDomSitename(document), License: extractLicense(document), DomURL: extractDomURL(document),
			OpenGraph: extractOpenGraphMeta(document), Meta: examineMeta(document),
			JSONLD: extractJsonLd(Options{}, document, examineMeta(document)),
			Extracted: extractMetadata(document, Options{HtmlDateMode: Disabled}),
			DomAuthor: extractDomAuthor(document), Categories: extractDomCategories(document), Tags: extractDomTags(document),
		})
	}
	for _, input := range []string{
		"", " ", "/article", "article", "../article", "//example.org/a", "///example.org/a", "?x=1", "#part",
		"https://example.org", "HTTP://EXAMPLE.ORG:80/a/../b", "https://www.example.org/a?x=1#part", "https://w12.example.org/",
		"https://user:pass@example.org:8443/a", "https://[::1]:8080/a", "http://[fe80::1%25eth0]/a", "https://example.org./a",
		"https://ex\u00e4mple.org/\u5b57", "https://example.org/a b?q=hello world#two words", "https://example.org/%2f/%2F?q=%zz",
		"https://example.org/%zz", "https://exa mple.org/", "https://example.org:bad/", "https://[::1/", "https://example.org/a\nb",
		"https:example.org/a", "https:///example.org/a", "http:/article", "mailto:person@example.org", "ftp://example.org/a",
		"data:text/plain,hello", "javascript:alert(1)", ":bad", "1bad:thing", "https://example.org/a\\b", "//example.org/%20a",
	} {
		absolute, _ := isAbsoluteURL(input)
		validated, validatedAbsolute := validateURL(input, nil)
		result.URLs = append(result.URLs, rustURLCase{input, absolute, getBaseURL(input), getDomainURL(input), validated, validatedAbsolute})
	}
	for _, base := range []string{"", "https://example.org/news/page?old=1#top", "https://example.org/root/", "https://example.org/a%2Fb/%FF", "https://user:pass@[::1]:8443/a/../b", "https:opaque"} {
		parsedBase, err := nurl.Parse(base)
		if err != nil {
			test.Fatal(err)
		}
		for _, input := range append(append([]string(nil), "./child", "child/../next", "%xx", "&value", "//other.example/path", "DATA:text/plain,hello"), func() []string {
			var values []string
			for _, entry := range result.URLs { values = append(values, entry.Input) }
			return values
		}()...) {
			validated, absolute := validateURL(input, parsedBase)
			result.URLResolution = append(result.URLResolution, rustURLResolutionCase{input, base, createAbsoluteURL(input, parsedBase), validated, absolute})
		}
	}
	entityFile, err := parser.ParseFile(token.NewFileSet(), filepath.Join(runtime.GOROOT(), "src", "html", "entity.go"), nil, 0)
	if err != nil { test.Fatal(err) }
	ast.Inspect(entityFile, func(node ast.Node) bool {
		entry, ok := node.(*ast.KeyValueExpr)
		if !ok { return true }
		literal, ok := entry.Key.(*ast.BasicLit)
		if !ok || literal.Kind != token.STRING { return true }
		name, err := strconv.Unquote(literal.Value)
		if err == nil { normalizationInputs = append(normalizationInputs, "&"+name, "prefix&"+name+"suffix") }
		return true
	})
	normalizationInputs = append(normalizationInputs,
		"&", "&#", "&#9", "&#9;", "&#9x", "&#99x", "&#x;", "&#X0;", "&#x80;", "&#128;", "&#0;", "&#55296;", "&#1114112;", "&#4294967361;",
		"&notit;", "&amp=word", "a\vb", `\\nr`, `a\u0041\ud83d\ude00b`, `John \"Nickname\" Doe`, "By JOHN DOE", "John Doe and Jane Smith", "iPhone News", "O'NEILL", "\u00dfeta \u03a3\u039f\u03a3", "Jane \U0001f469\u200d\U0001f4bb Doe",
	)
	seenNormalization := make(map[string]bool)
	for _, input := range normalizationInputs {
		if seenNormalization[input] { continue }
		seenNormalization[input] = true
		result.Normalization = append(result.Normalization, rustNormalizationCase{
			Input: input, Unescaped: html.UnescapeString(input), Cleaned: cleanMetadataText(input), JSON: normalizeJSONText(input),
			Tags: normalizeTags(input), Name: validateMetadataName(input), Authors: normalizeAuthors("", input),
			AuthorsExisting: normalizeAuthors("John Doe; Jane Smith", input), Title: cases.Title(language.English).String(input), NoEmoji: gomoji.RemoveEmojis(input),
		})
	}
	for _, input := range []string{
		`<html><head><meta property="article:published_time" content="2020-01-02T13:14:15+02:00"><meta property="article:modified_time" content="2021-03-04T15:16:17-05:00"></head><body></body></html>`,
		`<html><head><meta property="og:url" content="https://example.org/2017/09/01/content.html"></head></html>`,
		"<html><body><p>Ver\u00f6ffentlicht am 1.9.17</p></body></html>",
		`<html><body><div class="date">1 septembre 2017</div></body></html>`,
		`<html><body><div id="wm-ipp">2001-01-01</div><div class="date"><svg><text>1999-01-01</text></svg>2017-09-01</div></body></html>`,
		`<html><body><time datetime="2020-01-02">published</time></body></html>`,
		`<html><body>No date here</body></html>`,
		`<html><head><script type="application/ld+json">{"@context":"https://schema.org","@type":"Article","datePublished":"2018-05-06"}</script></head></html>`,
	} {
		for mode := 0; mode < 4; mode++ {
			for _, fallback := range []bool{false, true} {
				for custom := 0; custom < 3; custom++ {
					for override := 0; override < 3; override++ {
						document, err := html.Parse(strings.NewReader(input))
						if err != nil {
							test.Fatal(err)
						}
						originalURL, _ := nurl.Parse("https://example.org/article")
						opts := Options{EnableFallback: fallback, HtmlDateMode: HtmlDateMode(mode), OriginalURL: originalURL}
						if custom != 0 {
							opts.HtmlDateOptions = &htmldate.Options{ExtractTime: true, UseOriginalDate: custom == 1, SkipExtensiveSearch: custom == 1, URL: "https://ignored.example/2010/01/02", MinDate: time.Date(1995, 1, 1, 0, 0, 0, 0, time.UTC), MaxDate: time.Date(2025, 12, 31, 23, 59, 59, 0, time.UTC)}
						}
						if override != 0 {
							opts.HtmlDateOverride = &htmldate.Result{DateTime: time.Date(2012, 6, 7, 8, 9, 10, 0, time.FixedZone("", 7200)), HasTime: override == 2, HasTimezone: true}
						}
						before := dom.OuterHTML(document)
						metadata := extractMetadata(document, opts)
						if before != dom.OuterHTML(document) {
							test.Fatal("Go date extraction changed caller document")
						}
						result.Dates = append(result.Dates, rustDateCase{input, mode, fallback, custom, override, metadata.URL, metadata.Date.Format(time.RFC3339Nano)})
					}
				}
			}
		}
	}
	preparationInputs := []string{
		`<body><footer><p>Only paragraph</p></footer><div>body</div></body>`,
		`<body><figure><table><tr><td>Data</td></tr></table><figcaption>Caption</figcaption></figure><picture><source srcset="image.webp"><img src="image.jpg"></picture></body>`,
		`<body>first<span></span>tail<div><b></b></div>last<!--comment--><p>keep</p></body>`,
		`<body><table role="presentation"><tr><td>Layout</td></tr></table><p><mark>marked</mark> <img src="a.jpg"> tail</p></body>`,
	}
	for _, input := range result.DOM {
		if input.Operation == "inspect" {
			preparationInputs = append(preparationInputs, input.HTML)
		}
	}
	for _, input := range preparationInputs {
		for focus := Balanced; focus <= FavorPrecision; focus++ {
			for _, excludeTables := range []bool{false, true} {
				for _, includeImages := range []bool{false, true} {
					document, err := html.Parse(strings.NewReader(input))
					if err != nil { test.Fatal(err) }
					docCleaning(document, Options{Focus: focus, ExcludeTables: excludeTables, IncludeImages: includeImages})
					result.Preparation = append(result.Preparation, rustPreparationCase{input, focus, excludeTables, includeImages, dom.OuterHTML(document)})
				}
			}
		}
	}
	conversionInputs := append([]string(nil), preparationInputs...)
	conversionInputs = append(conversionInputs,
		`<body><a href="/loose">Loose</a><div><a href="/kept">Kept <span><img src="a.jpg"> image tail</span></a> link tail<p><sup></sup>sup tail<sub>2</sub><strong class="schema-faq-question other" id="question">Question</strong></p></div></body>`,
		"<pre><span class=\"ordinary\">code</span></pre><blockquote><span class=\"language hljs-key\" title=\"drop\">highlighted</span></blockquote><q><span class=\"xhljs\">quote</span></q><pre>{\n    code}</pre><pre>ordinary text</pre>",
		`<p><a href=" ../page?query=a#frag " target=" _blank " class="drop">Relative</a><a href="javascript:alert(1)" target=" #here ">Special</a><a href="https://example.net/abs" title="drop">Absolute</a></p><nav><a href="/nav">navigation</a></nav>`,
		`<body><a href="/gallery"><img src="one.png"><!--between--> first tail <img src="two.png">second tail</a>outside tail<b>next</b><table><tr><td><a href="row">cell</a></td></tr></table><a href="last"><span><img src="last.png"></span></a></body>`,
	)
	for _, input := range conversionInputs {
		for _, excludeTables := range []bool{false, true} {
			for _, includeImages := range []bool{false, true} {
				for _, includeLinks := range []bool{false, true} {
					for _, original := range []string{"", "https://example.org/path/entry?old=1#old", "https://example.org/%FF/page"} {
						document, err := html.Parse(strings.NewReader(input))
						if err != nil { test.Fatal(err) }
						var originalURL *nurl.URL
						if original != "" { originalURL, _ = nurl.Parse(original) }
						convertTags(document, Options{ExcludeTables: excludeTables, IncludeImages: includeImages, IncludeLinks: includeLinks, OriginalURL: originalURL})
						result.Conversion = append(result.Conversion, rustConversionCase{input, excludeTables, includeImages, includeLinks, original, dom.OuterHTML(document)})
					}
				}
			}
		}
	}
	densityInputs := []string{
		`<div><a>one</a><a>two</a><a>three</a><span>some plain words</span></div>`,
		`<ul><li><p><a>one</a><a>two</a></p></li></ul><table><tr><td><p><a>one</a><a>two</a></p></td></tr></table>`,
		`<div><p><a></a><a> </a></p>tail<p><a>one</a><img src="one.png"></p></div>`,
		"<p>" + strings.Repeat("plain", 4) + "<a>" + strings.Repeat("linked", 15) + "</a></p>",
	}
	for _, length := range []int{0, 9, 10, 29, 30, 59, 60, 99, 100, 199, 200, 299, 300, 999, 1000} {
		for _, count := range []int{1, 2, 5} {
			links := strings.Repeat("<a>"+strings.Repeat("x", length/count)+"</a>", count)
			for _, next := range []string{"", "<p>next</p>"} {
				densityInputs = append(densityInputs, "<div><p>"+links+"</p>"+next+"<table><tr><td>"+links+"</td></tr></table></div>")
			}
		}
	}
	for _, input := range densityInputs {
		for focus := Balanced; focus <= FavorPrecision; focus++ {
			for _, images := range []bool{false, true} {
				document, err := html.Parse(strings.NewReader(input))
				if err != nil { test.Fatal(err) }
				opts := Options{Focus: focus, IncludeImages: images}
				entry := rustLinkDensityCase{HTML: input, Focus: focus, IncludeImages: images}
				for _, node := range dom.GetElementsByTagName(document, "*") {
					length, short, nonEmpty := collectLinkInfo(dom.GetElementsByTagName(node, "a"))
					selected, high := linkDensityTest(node, opts)
					entry.Nodes = append(entry.Nodes, rustLinkDensityNode{length, short, len(nonEmpty), len(selected), high, linkDensityTestTables(node, opts)})
				}
				for _, backtracking := range []bool{false, true} {
					clone := dom.Clone(document, true)
					deleteByLinkDensity(clone, opts, backtracking, "div", "p", "ul", "table")
					entry.Deleted = append(entry.Deleted, dom.OuterHTML(clone))
				}
				result.LinkDensity = append(result.LinkDensity, entry)
			}
		}
	}
	textNodeInputs := []string{
		`<p id="probe"> ordinary <b>nested</b> tail </p> outside`,
		`<p id="probe"></p> tail`, `<br id="probe"> tail`, `<hr id="probe"> tail`,
		`<lb id="probe"></lb> tail`, `<done id="probe">content</done>`,
		`<img id="probe" src="image.jpg"> tail`, `<img id="probe"> tail`,
		`<div id="probe"><p>nested only</p></div>`,
	}
	for _, text := range []string{"", " \t\n ", "Facebook", "prefixFacebook", "Facebook ", "print\nbody", "body\nPrint", "More on this........", "More on this.........", "Mehr zum Thema:", "L\u0130NKEDIN", "Print\x00", "x\x00Print", "author\r\nTwitter"} {
		textNodeInputs = append(textNodeInputs, `<p id="probe">`+text+`</p>tail`)
	}
	for _, input := range textNodeInputs {
		for _, fix := range []bool{false, true} {
			for _, spaces := range []bool{false, true} {
				for _, light := range []bool{false, true} {
					for _, capacity := range []int{-1, 0, 1, 2} {
						document, err := html.Parse(strings.NewReader(input))
						if err != nil { test.Fatal(err) }
						probe := dom.GetElementByID(document, "probe")
						cache := lru.NewCache(capacity)
						config := DefaultConfig()
						config.MinDuplicateCheckSize = 0
						opts := Options{Deduplicate: true, Config: config}
						entry := rustTextNodeCase{HTML: input, Fix: fix, Spaces: spaces, Light: light, Capacity: capacity, Filtered: textFilter(probe)}
						for iteration := 0; iteration < 6; iteration++ {
							var processed *html.Node
							if light { processed = processNode(probe, cache, opts) } else { processed = handleTextNode(probe, cache, fix, spaces, opts) }
							entry.Accepted = append(entry.Accepted, processed != nil)
							if iteration == 3 { cache.Put("different cache entry", 1) }
						}
						entry.Output = dom.OuterHTML(document)
						result.TextNodes = append(result.TextNodes, entry)
					}
				}
			}
		}
	}
	for _, input := range [][2]string{
		{"title", `<h2 id="probe">Heading <em>inline <b>nested</b></em> end</h2>tail`},
		{"title", `<summary id="probe">Summary</summary>tail`},
		{"title", `<h3 id="probe"><span></span></h3>`},
		{"list", `<ul id="probe">initial<li>first</li>tail<li><p>second <a href="../x"><b>linked</b> tail</a></p><ul><li>nested</li></ul>end</li>last</ul>`},
		{"list", `<dl id="probe"><dt>term</dt><dd>definition <em>emphasis</em><br> tail</dd></dl>`},
		{"list", `<ol id="probe"><li> </li><li><img src="image.png"></li></ol>`},
		{"quote", `<blockquote id="probe">Initial<p>paragraph <a href="/x"><em>linked</em></a> tail<img src="/photo.png" alt="photo"></p><q>nested quote</q></blockquote>outside`},
		{"quote", `<pre id="probe" lang="python"> a\n<b class="drop"> b </b> c </pre>tail`},
		{"quote", `<div class="highlight"><pre id="probe"> x <span>code</span> </pre></div>`},
		{"quote", `<pre id="probe"><code>inside <b>bold</b></code></pre>`},
		{"paragraph", `<p id="probe">one <strong><span>two</span><br>three</strong> four<a href="/link"><b>bold</b> linked</a><img data-src="/lazy.webp" src="pixel.gif">tail<br></p>`},
		{"paragraph", `<p id="probe"><em><span>one</span><span>two</span></em><a><span>only span</span></a><del>deleted</del><code><span>code</span></code></p>`},
		{"paragraph", `<p id="probe">Print</p>tail`},
		{"formatting", `<div><strong id="probe">orphan <i>nested</i></strong>tail</div>`},
		{"formatting", `<p><a id="probe" href="/link">link</a>tail</p>`},
		{"image", `<img id="probe" src="pixel.gif" data-src="//example.org/photo.jpg" alt="caption" title="title">tail`},
		{"image", `<img id="probe" src="missing" data-src-lazy="../other.webp">tail`},
		{"image", `<img id="probe" alt="no source">tail`},
		{"other", `<div id="probe">leading <p>paragraph</p>tail</div>`},
		{"other", `<details id="probe">leading<summary>summary</summary>detail</details>`},
		{"other", `<div id="probe" class="w3-code">  code <span>line</span> </div>`},
		{"table", `<table id="probe"><caption>Table <b>caption</b></caption><thead><tr><th>first</th><th>second</th></tr></thead><tbody><tr><th>row header</th><td>cell</td></tr><tr><td>short</td></tr></tbody></table>`},
		{"table", `<table id="probe"><tr><th colspan="2" rowspan="3">header</th><td>other</td></tr><tr><td>second</td></tr><tr><th>third</th></tr><tr><td colspan="0">zero</td><td>next</td></tr></table>`},
		{"table", `<table id="probe"><tr><td>before <em><b>inline</b></em><table><tr><td>nested data</td></tr></table> after <p>paragraph <a href="/link">linked</a></p><ul><li>list</li><li>more</li></ul></td><td><img src="image.webp">tail</td></tr></table>`},
		{"table", `<table id="probe"><tr><td></td></tr><tr><td> </td></tr></table>`},
		{"table", "<table id=\"probe\"><tr><td colspan=\"\u0662\" rowspan=\"\uff12\">unicode span</td><td colspan=\"invalid\">plain</td></tr><tr><td>next</td></tr></table>"},
		{"text", `<br id="probe">tail`},
		{"text", `<section id="probe">unknown</section>`},
	} {
		for focus := Balanced; focus <= FavorPrecision; focus++ {
			for _, images := range []bool{false, true} {
				for _, links := range []bool{false, true} {
					for _, deduplicate := range []bool{false, true} {
						document, err := html.Parse(strings.NewReader(input[1]))
						if err != nil { test.Fatal(err) }
						probe := dom.GetElementByID(document, "probe")
						originalURL, _ := nurl.Parse("https://example.org/path/article")
						opts := Options{Focus: focus, IncludeImages: images, IncludeLinks: links, Deduplicate: deduplicate, Config: DefaultConfig(), OriginalURL: originalURL}
						cache := lru.NewCache(opts.Config.CacheSize)
						potential := sliceToMap("div", "table", "tr", "th", "td")
						for tag := range tagCatalog { potential[tag] = struct{}{} }
						if images { potential["img"] = struct{}{} }
						if links { potential["a"] = struct{}{} }
						var processed *html.Node
						switch input[0] {
						case "title": processed = handleTitles(probe, cache, opts)
						case "list": processed = handleLists(probe, cache, opts)
						case "quote": processed = handleQuotes(probe, cache, opts)
						case "paragraph": processed = handleParagraphs(probe, potential, cache, opts)
						case "formatting": processed = handleFormatting(probe, cache, opts)
						case "image": processed = handleImage(probe, opts)
						case "other": processed = handleOtherElements(probe, potential, cache, opts)
						case "table": processed = handleTable(probe, potential, cache, opts)
						case "text": processed = handleTextElem(probe, potential, cache, opts)
						}
						result.Handlers = append(result.Handlers, rustHandlerCase{input[1], input[0], focus, images, links, deduplicate, etree.ToString(processed), dom.OuterHTML(document)})
					}
				}
			}
		}
	}
	spanInputs := []string{"", "0", "00", "1", "99", "100", "101", "99999999999999999999999", "9999999999999x", " 2", "2 ", "-1", "+2", "2.0", "one"}
	for scalar := rune(0); scalar <= unicode.MaxRune; scalar++ {
		if unicode.IsDigit(scalar) {
			spanInputs = append(spanInputs, string(scalar), "1"+string(scalar), string(scalar)+"2")
		}
	}
	for _, value := range spanInputs {
		cell := dom.CreateElement("td")
		dom.SetAttribute(cell, "colspan", value)
		result.Spans = append(result.Spans, rustSpanCase{value, tableSpan(cell, "colspan")})
	}
	ruleGroups := [][]selector.Rule{selector.Content, selector.OverallDiscardedContent, selector.PrecisionDiscardedContent, selector.Comments, selector.DiscardedComments, selector.RemovedComments, selector.DiscardedImage, selector.DiscardedTeaser}
	signals := map[string]bool{"": true, "x cookie y": true, "x link y": true, "x\u00a0link\u00a0y": true, "x\u200blink\u200by": true, "body": true, "true": true}
	for _, filename := range []string{"content.go", "content-discard-overall.go", "content-discard-precision.go", "comments.go", "comments-discard.go", "comments-removed.go", "image-discard.go", "teaser-discard.go"} {
		source, err := parser.ParseFile(token.NewFileSet(), filepath.Join("internal", "selector", filename), nil, 0)
		if err != nil { test.Fatal(err) }
		ast.Inspect(source, func(node ast.Node) bool {
			if literal, ok := node.(*ast.BasicLit); ok && literal.Kind == token.STRING {
				value, err := strconv.Unquote(literal.Value)
				if err == nil { signals[value] = true; signals[strings.ToUpper(value)] = true }
			}
			return true
		})
	}
	var sortedSignals []string
	for value := range signals { sortedSignals = append(sortedSignals, value) }
	sort.Strings(sortedSignals)
	for _, tag := range []string{"div", "article", "p", "main", "li", "header", "details", "cite", "quote", "span", "h2"} {
		for _, value := range sortedSignals {
			for layout := 0; layout < 6; layout++ {
				node := dom.CreateElement(tag)
				switch layout {
				case 0: dom.SetAttribute(node, "id", value)
				case 1: dom.SetAttribute(node, "class", value)
				case 2: dom.SetAttribute(node, "id", "unrelated"); dom.SetAttribute(node, "class", value)
				case 3: dom.SetAttribute(node, "class", value); dom.SetAttribute(node, "id", "unrelated")
				case 4:
					for _, key := range []string{"style", "role", "itemprop", "data-component", "aria-hidden"} { dom.SetAttribute(node, key, value) }
					dom.SetAttribute(node, value, "")
				case 5: dom.SetAttribute(node, "id", "\u00a0"+value+"\t"); dom.SetAttribute(node, "class", "\u00a0"+value+"\t")
				}
				entry := rustSelectorCase{Tag: tag, Value: value, Layout: layout}
				for _, rules := range ruleGroups { for _, rule := range rules { entry.Matches = append(entry.Matches, rule(node)) } }
				result.Selectors = append(result.Selectors, entry)
			}
		}
	}
	pruningInputs := append([]string(nil), preparationInputs...)
	pruningInputs = append(pruningInputs,
		`<article><div class="related"><p>all the main content was unexpectedly marked related</p></div><p>x</p></article>`,
		`<article><p>Main text with enough length to retain while the footer is removed.</p><div class="footer">footer</div>footer tail<header>header</header><p class="teaser">teaser</p></article><section class="comments"><p>comment</p><div id="respond">response form</div></section>`,
	)
	for _, input := range pruningInputs {
		for group, rules := range ruleGroups {
			for _, backup := range []bool{false, true} {
				document, err := html.Parse(strings.NewReader(input))
				if err != nil { test.Fatal(err) }
				processed := pruneUnwantedNodes(document, rules, backup)
				result.Pruning = append(result.Pruning, rustPruningCase{input, group, backup, dom.OuterHTML(processed), dom.OuterHTML(document)})
			}
		}
	}
	processingInputs := append([]string(nil), pruningInputs...)
	processingInputs = append(processingInputs,
		`<article><h1>Title</h1><p>`+strings.Repeat("Article text that is long enough to select and retain. ", 8)+`</p><p>Second paragraph with <em>inline text</em>.</p><div class="related"><p>Related story</p></div><h2>Trailing heading</h2></article><section id="comments"><div class="comment"><p>First comment</p><a>reply link</a></div><p>Second comment</p><div id="respond"><p>Reply form</p></div></section>`,
		`<main><div class="post-content"><p>First story paragraph.</p></div></main><section><p>`+strings.Repeat("Wild text outside the chosen frame. ", 10)+`</p><p class="teaser">Teaser text</p></section><div class="comment-list"><p>A comment</p></div>`,
		`<div class="article-body">First line<br>Second line<br>Third line</div><div class="comments-content"><p>Facebook</p><p>Content comment</p></div>`,
		`<main><p>`+strings.Repeat("Duplicate long paragraph text. ", 4)+`</p><p>`+strings.Repeat("Duplicate long paragraph text. ", 4)+`</p><figure><img src="image.jpg" alt="photo"></figure><table><tr><td>data</td></tr></table></main>`,
	)
	seenProcessing := make(map[string]bool)
	for _, input := range processingInputs {
		if seenProcessing[input] { continue }; seenProcessing[input] = true
		for focus := Balanced; focus <= FavorPrecision; focus++ {
			for flags := 0; flags < 32; flags++ {
				document, err := html.Parse(strings.NewReader(input))
				if err != nil { test.Fatal(err) }
				originalURL, _ := nurl.Parse("https://example.org/path/article")
				opts := Options{Focus: focus, IncludeImages: flags&1 != 0, IncludeLinks: flags&2 != 0, Deduplicate: flags&4 != 0, EnableFallback: flags&8 != 0, ExcludeComments: flags&16 != 0, Config: DefaultConfig(), OriginalURL: originalURL}
				cache := lru.NewCache(opts.Config.CacheSize)
				docCleaning(document, opts)
				convertTags(document, opts)
				var comments *html.Node
				var commentsText string
				if !opts.ExcludeComments { comments, commentsText = extractComments(document, cache, opts) }
				content, contentText := extractContent(document, cache, opts)
				result.Content = append(result.Content, rustContentCase{input, focus, flags, dom.OuterHTML(content), contentText, dom.OuterHTML(comments), commentsText, dom.OuterHTML(document)})
			}
		}
	}
	baselineInputs := append([]string(nil), processingInputs...)
	baselineInputs = append(baselineInputs,
		`<script type="application/ld+json">{"articleBody":"`+strings.Repeat("JSON article text. ", 10)+`","@graph":[{"reviewBody":"review"}],"mainEntity":{"acceptedAnswer":{"text":"answer"}}}</script><body>short</body>`,
		`<script type="application/ld+json">[{"@type":"Recipe","recipeInstructions":["one",{"text":"two","itemListElement":[{"text":"three"}]}],"@graph":{"step":["four",{"text":"five"}]},"mainEntity":{"articleBody":"six"}},{"@type":["Unknown","Product"],"description":"`+strings.Repeat("Product description. ", 8)+`"}]</script><p>short</p>`,
		`<script type="application/ld+json">{"@type":"Product","description":"&lt;p&gt;`+strings.Repeat("Embedded description. ", 8)+`&lt;/p&gt;"}</script><div class="cookie-consent">cookie</div>body`,
		`<article>`+strings.Repeat("Large article. ", 100)+`<article>nested article</article></article><article>`+strings.Repeat("short article ", 8)+`</article><div class="ONETRUST">consent</div>`,
		`<div id="data-preloaded" data-preloaded='{"topic_2":"{\"post_stream\":{\"posts\":[{\"cooked\":\"&lt;p&gt;second&lt;/p&gt;\"}]}}","topic_1":"{\"post_stream\":{\"posts\":[{\"cooked\":\"&lt;p&gt;first&lt;/p&gt;\"}]}}"}'></div>`,
	)
	for _, input := range baselineInputs {
		document, err := html.Parse(strings.NewReader(input))
		if err != nil { test.Fatal(err) }
		before := dom.OuterHTML(document)
		bodies, teasers := collectJSONContent(document)
		body, content := baseline(document)
		flat := html2txt(document)
		if before != dom.OuterHTML(document) { test.Fatal("baseline changed caller") }
		cleaned := basicCleaning(dom.Clone(document, true))
		result.Baseline = append(result.Baseline, rustBaselineCase{input, bodies, teasers, dom.OuterHTML(body), content, plainText(body), flat, dom.OuterHTML(cleaned)})
	}
	for _, value := range []string{"plain <angular> code", "<p>one</p><p>two</p>", "<p class='one'>text &amp; more</p>", "&lt;p&gt;escaped&lt;/p&gt;", "<html><head><title>ignored</title></head><body>body</body></html>", "<p bare>not detected", "<br>separated", "<p>one\x00two\u200bthree</p>"} {
		result.BaselineText = append(result.BaselineText, rustStringCase{value, renderBaselineText(value)})
	}
	postCleaningInputs := append([]string(nil), processingInputs...)
	var attributeNames []string
	for name := range allowedAttributes { attributeNames = append(attributeNames, name) }
	attributeNames = append(attributeNames, "onerror", "onclick", "data-extra", "aria-label", "unknown")
	sort.Strings(attributeNames)
	for _, tag := range []string{"p", "table", "th", "td", "hr", "pre", "img", "a", "span", "div", "code"} {
		element := dom.CreateElement(tag)
		for _, name := range attributeNames { dom.SetAttribute(element, name, "value") }
		if !dom.IsVoidElement(element) { etree.SetText(element, "content") }
		input := dom.OuterHTML(element)
		if tag == "th" || tag == "td" { input = "<table><tr>"+input+"</tr></table>" }
		postCleaningInputs = append(postCleaningInputs, input)
	}
	postCleaningInputs = append(postCleaningInputs, `<div>first<span> </span> tail<p><b></b><i></i></p><table><tr><td></td><th></th></tr></table><img src="photo.jpg" width="20"><br></div>`)
	for _, input := range postCleaningInputs {
		document, err := html.Parse(strings.NewReader(input))
		if err != nil { test.Fatal(err) }
		postCleaning(document)
		result.PostCleaning = append(result.PostCleaning, rustStringCase{input, dom.OuterHTML(document)})
	}
	candidateInputs := []string{"", "<p></p>", "<div>short</div>", "<p>"+strings.Repeat("abc ", 25)+"</p>", "<p>"+strings.Repeat("abc ", 38)+"</p>", "<p>"+strings.Repeat("abc ", 51)+"</p>", "<h2>Heading</h2><p>"+strings.Repeat("abc ", 28)+"</p>", "<h1>Heading</h1><p>"+strings.Repeat("abc ", 150)+"</p>", "<div>{"+strings.Repeat("raw json ", 90)+"}</div>", "<table><tr><td>"+strings.Repeat("table text ", 60)+"</td></tr></table>"}
	for _, candidateHTML := range candidateInputs {
		candidate, _ := html.Parse(strings.NewReader(candidateHTML))
		candidateLength := utf8.RuneCountInString(trim(etree.IterText(candidate, " ")))
		for _, extractedHTML := range candidateInputs {
			extracted, _ := html.Parse(strings.NewReader(extractedHTML))
			extractedLength := utf8.RuneCountInString(trim(etree.IterText(extracted, " ")))
			for _, focus := range []ExtractionFocus{Balanced, FavorRecall, FavorPrecision} {
				for _, minimum := range []int{1, 50, 250} {
					config := *DefaultConfig()
					config.MinExtractedSize = minimum
					usable := candidateIsUsable(candidate, extracted, candidateLength, extractedLength, Options{Config: &config, Focus: focus})
					result.FallbackSelection = append(result.FallbackSelection, rustFallbackSelectionCase{candidateHTML, extractedHTML, focus, minimum, candidateLength, extractedLength, usable})
				}
			}
		}
	}
	sanitizeInputs := append([]string(nil), processingInputs...)
	sanitizeInputs = append(sanitizeInputs, `<div><custom>unknown <span>span <tt>tt</tt></span></custom><noindex>hidden</noindex>tail<img src="photo.jpg"><fencedframe>frame</fencedframe>after</div>`, `<table><tr><th>first</th></tr><tr><th>second<table><tr><th>nested first</th></tr><tr><th>nested second</th></tr></table></th></tr></table>`)
	for _, input := range sanitizeInputs {
		for _, focus := range []ExtractionFocus{Balanced, FavorRecall, FavorPrecision} {
			for flags := 0; flags < 8; flags++ {
				document, _ := html.Parse(strings.NewReader(input))
				sanitizeTree(document, Options{Config: DefaultConfig(), Focus: focus, IncludeImages: flags&1 != 0, IncludeLinks: flags&2 != 0, ExcludeTables: flags&4 != 0})
				result.Sanitization = append(result.Sanitization, rustSanitizationCase{input, focus, flags, dom.OuterHTML(document)})
			}
		}
	}
	fallbackInputs := append([]string(nil), processingInputs...)
	fallbackInputs = append(fallbackInputs, `<article><h1>Native fallback</h1><p>`+strings.Repeat("Article paragraph, with several words and punctuation. ", 20)+`</p><p>`+strings.Repeat("Second article paragraph, more details about the subject. ", 20)+`</p></article><aside><p>aside</p></aside>`, `<div class="comments"><p>`+strings.Repeat("Comment text, with content. ", 80)+`</p></div><fencedframe><p>frame content</p></fencedframe><article><p>`+strings.Repeat("Article text. ", 35)+`</p></article>`)
	for _, input := range fallbackInputs {
		for _, focus := range []ExtractionFocus{Balanced, FavorRecall, FavorPrecision} {
			for flags := 0; flags < 4; flags++ {
				for variant := 0; variant < 4; variant++ {
					document, _ := html.Parse(strings.NewReader(input))
					original := dom.OuterHTML(document)
					extractedHTML := "<p>seed extraction text.</p>"
					if variant == 3 { extractedHTML = "<h1>Long seed</h1><p>"+strings.Repeat("existing extraction ", 150)+"</p>" }
					extracted, _ := html.Parse(strings.NewReader(extractedHTML))
					extracted = dom.QuerySelector(extracted, "body")
					options := Options{Config: DefaultConfig(), Focus: focus, IncludeImages: flags&1 != 0, IncludeLinks: flags&2 != 0}
					options.OriginalURL, _ = nurl.Parse("https://example.com/news/page")
					if variant != 0 {
						candidate, _ := html.Parse(strings.NewReader("<div><h2>Custom candidate</h2><p>"+strings.Repeat("Custom candidate text. ", 20)+"</p><a href='../more'>more</a><img src='image.jpg'></div>"))
						candidate = dom.QuerySelector(candidate, "div")
						options.FallbackCandidates = &FallbackCandidates{}
						if variant == 1 { options.FallbackCandidates.Others = []*html.Node{nil, dom.CreateElement("div"), candidate} }
						if variant == 2 { options.FallbackCandidates.Readability = candidate; options.FallbackCandidates.Distiller = candidate }
					}
					var candidateBefore []string
					if options.FallbackCandidates != nil {
						for _, candidate := range append(options.FallbackCandidates.Others, options.FallbackCandidates.Readability, options.FallbackCandidates.Distiller) { candidateBefore = append(candidateBefore, dom.OuterHTML(candidate)) }
					}
					body, content := compareExternalExtraction(document, extracted, options)
					rescued, rescuedText := distillerRescue(document, options)
					if dom.OuterHTML(document) != original { test.Fatal("fallback changed original document") }
					if options.FallbackCandidates != nil {
						for index, candidate := range append(options.FallbackCandidates.Others, options.FallbackCandidates.Readability, options.FallbackCandidates.Distiller) {
							if candidateBefore[index] != dom.OuterHTML(candidate) { test.Fatal("fallback changed custom candidate") }
						}
					}
					result.NativeFallbacks = append(result.NativeFallbacks, rustNativeFallbackCase{input, extractedHTML, focus, flags, variant, dom.OuterHTML(body), content, dom.OuterHTML(rescued), rescuedText})
				}
			}
		}
	}
	for _, input := range []string{"", `<html lang="fr-FR">`, `<html lang="">`, `<html lang="EN en_US">`, `<meta http-equiv="content-language" content="fr"><meta property="og:locale" content="en">`, `<meta http-equiv="content-language" content="fr"><meta http-equiv="content-language" content="en-US">`, `<meta property="og:locale" content="de, EN, ja">`, `<meta http-equiv="Content-Language" content="fr"><html lang="en">`, `<meta http-equiv="content-language"><meta property="og:locale" content="en">`, `<meta http-equiv="content-language" content="">`, `<meta property="og:locale" content="english french">`, "<html lang='\u212ae \u017fn'>"} {
		document, _ := html.Parse(strings.NewReader(input))
		for _, target := range []string{"en", "EN", "fr", "de", "ja", "ke", "sn", ""} {
			for _, strict := range []bool{false, true} {
				result.LanguageGates = append(result.LanguageGates, rustLanguageGateCase{input, target, strict, checkHtmlLanguage(document, Options{TargetLanguage: target}, strict)})
			}
		}
	}
	languageInputs := []string{"", "1234", "This is an English article about the development of software and scientific research.", "Ceci est un article en fran\u00e7ais sur le d\u00e9veloppement des logiciels et la recherche scientifique.", "Dies ist ein deutscher Artikel \u00fcber die Entwicklung von Software und wissenschaftliche Forschung.", "Este es un art\u00edculo en espa\u00f1ol sobre el desarrollo de programas y la investigaci\u00f3n cient\u00edfica.", "\u65e5\u672c\u8a9e\u306e\u6587\u7ae0\u3092\u8aad\u307f\u307e\u3059\u3002", "caf\u00e9", "abcd"}
	for _, content := range languageInputs {
		for _, comments := range languageInputs { result.Languages = append(result.Languages, rustLanguageCase{content, comments, languageClassifier(content, comments)}) }
	}
	for _, value := range []string{`null`, `{"@type":"DiscussionForumPosting"}`, `{"@type":["Article"," DiscussionForumPosting "]}`, `{"@graph":{"mainEntity":[{"child":{"@type":"DISCUSSIONFORUMPOSTING"}}]}}`, `[{"@type":"Article"},[{"@type":"DiscussionForumPosting"}]]`, `{"@type":"https://schema.org/DiscussionForumPosting"}`, `{"unrelated":"DiscussionForumPosting"}`, `{"@type":5}`, `{"@type":"DiscussionForumPosting",}`, "{\"@type\":\"DiscussionForumPosting\",\"text\":\"raw\nnewline\"}"} {
		for _, kind := range []string{"application/ld+json", "application/json", "APPLICATION/LD+JSON"} {
			input := `<script type="`+kind+`">`+value+`</script>`
			document, _ := html.Parse(strings.NewReader(input))
			result.Forums = append(result.Forums, rustForumCase{input, forumThreadPage(document)})
		}
	}
	sequenceInputs := append([]string(nil), fallbackInputs...)
	sequenceInputs = append(sequenceInputs, `<script type="application/ld+json">{"@type":"DiscussionForumPosting"}</script><article><p>`+strings.Repeat("Initial forum post. ", 30)+`</p></article><div id="comments"><p>`+strings.Repeat("First forum reply. ", 25)+`</p><p>`+strings.Repeat("Second forum reply. ", 25)+`</p></div>`, `<div class="post-content"><p>`+strings.Repeat("Main short paragraph. ", 15)+`</p></div><div><p>`+strings.Repeat("Additional article detail with longer paragraphs and words. ", 250)+`</p></div><div id="comments"><p>Do not include comments in retry.</p></div>`)
	for _, input := range sequenceInputs {
		for _, focus := range []ExtractionFocus{Balanced, FavorRecall, FavorPrecision} {
			for flags := 0; flags < 64; flags++ {
				document, _ := html.Parse(strings.NewReader(input))
				options := Options{Config: DefaultConfig(), Focus: focus, IncludeImages: flags&1 != 0, IncludeLinks: flags&2 != 0, Deduplicate: flags&4 != 0, EnableFallback: flags&8 != 0, ExcludeComments: flags&16 != 0, ExcludeTables: flags&32 != 0}
				options.OriginalURL, _ = nurl.Parse("https://example.com/news/page")
				body, content, comments, commentsText := extractionSequence(document, lru.NewCache(options.Config.CacheSize), options)
				result.Sequences = append(result.Sequences, rustContentCase{input, focus, flags, dom.OuterHTML(body), content, dom.OuterHTML(comments), commentsText, dom.OuterHTML(document)})
			}
		}
	}
	cssInputs := []string{
		`<html lang="en"><body><main id="Main" class="article foo"><p class="foo bar" data-x="one Two-three">First <b>bold</b> tail</p><p id="number:1" class="bar">Second</p><!--between--><div><p>Nested</p></div><p class="foo">Third</p><span> </span><a href="/page" lang="fr-CA">link</a><input type="CHECKBOX" checked><input disabled><select><optgroup disabled><option selected>choice</option></optgroup></select></main></body></html>`,
		`<fieldset disabled><legend><input id="first"></legend><legend><input id="second"></legend><div><input id="third"></div><fieldset><button>button</button></fieldset></fieldset><div lang="EN"><span lang="fr"><b>inherited</b></span></div>`,
		"<p id='\u212a' data-x='\u017f \u03c2 \u0130'>caf\u00e9 \u65e5\u672c\u8a9e</p><p data-x=''></p><p data-x='  '></p><p>empty attributes</p>",
	}
	cssSelectors := []string{"*", "*|*", "*|* ", "p", "P", ".foo", ".bar", "#Main", `#number\:1`, `[data-x]`, `[data-x=""]`, `[data-x!=""]`, `[data-x~=""]`, `[data-x~="two-three" i]`, `[data-x|="one"]`, `[data-x^=""]`, `[data-x$=""]`, `[data-x*="TWO" i]`, `[data-x#=(one|Two)]`, `[id="k" i]`, `[data-x~="s" i]`, `[data-x~="\3c3" i]`, `[data-x~="i" i]`, "main p", "main > p", "p + p", "p ~ p", "main/*comment*/p", "p:not(.foo)", "main:has(p.bar)", "main:haschild(p.bar)", "p:contains(First)", `p:contains("bold tail")`, "p:containsown(tail)", "p:matches(First.*tail)", "p:matchesown(First.*tail)", `p:matches(\w+)`, "p:nth-child(2n+1)", "p:nth-last-child(-n+3)", "p:nth-of-type(odd)", "p:nth-last-of-type(EVEN)", "p:nth-child(+n - 1)", "p:nth-child(0)", "p:nth-child(999999999999999999999)", ":first-child", ":last-child", ":first-of-type", ":last-of-type", ":only-child", ":only-of-type", ":input", ":empty", ":root", ":link", ":lang(en)", ":lang(fr)", ":enabled", ":disabled", ":checked", ":visited", ":hover", ":active", ":focus", ":target", "p,div", "", " ", "p,", "p >", "p::before", "p:before", "p:hover()", "[x=4]", "[x='4' s]", "p:has(> p)", "p:contains(two words)", "p:nth-child(n+)", "p|span", "/*unclosed", "p:unknown", "p:not(:has(span), .foo)"}
	for _, input := range cssInputs {
		for _, source := range cssSelectors {
			document, _ := html.Parse(strings.NewReader(input))
			compiled, err := cascadia.ParseGroup(source)
			item := rustCSSCase{HTML: input, Selector: source, Valid: err == nil}
			if err == nil {
				for _, element := range dom.GetElementsByTagName(document, "*") { if compiled.Match(element) { item.Matches = append(item.Matches, dom.OuterHTML(element)) } }
				pruneUnwantedNodes(document, []selector.Rule{compiled.Match})
			}
			item.Pruned = dom.OuterHTML(document)
			result.CSS = append(result.CSS, item)
		}
	}
	addCSSCase := func(input, source string) {
		document, _ := html.Parse(strings.NewReader(input))
		compiled, err := cascadia.ParseGroup(source)
		item := rustCSSCase{HTML: input, Selector: source, Valid: err == nil}
		if err == nil {
			for _, element := range dom.GetElementsByTagName(document, "*") { if compiled.Match(element) { item.Matches = append(item.Matches, dom.OuterHTML(element)) } }
			pruneUnwantedNodes(document, []selector.Rule{compiled.Match})
		}
		item.Pruned = dom.OuterHTML(document)
		result.CSS = append(result.CSS, item)
	}
	cssDirectory := os.Getenv("RUST_REFERENCE_CSS_SOURCE")
	cssSource, err := parser.ParseFile(token.NewFileSet(), filepath.Join(cssDirectory, "selector_test.go"), nil, 0)
	if err != nil { test.Fatal(err) }
	ast.Inspect(cssSource, func(node ast.Node) bool {
		declaration, ok := node.(*ast.ValueSpec)
		if !ok || len(declaration.Names) != 1 || declaration.Names[0].Name != "selectorTests" { return true }
		for _, value := range declaration.Values[0].(*ast.CompositeLit).Elts {
			fields := value.(*ast.CompositeLit).Elts
			input, err := strconv.Unquote(fields[0].(*ast.BasicLit).Value)
			if err != nil { test.Fatal(err) }
			source, err := strconv.Unquote(fields[1].(*ast.BasicLit).Value)
			if err != nil { test.Fatal(err) }
			addCSSCase(input, source)
		}
		return false
	})
	cssDocument, err := os.ReadFile(filepath.Join(cssDirectory, "test_resources/content.xhtml"))
	if err != nil { test.Fatal(err) }
	for _, filename := range []string{"valid_selectors.json", "invalid_selectors.json"} {
		data, err := os.ReadFile(filepath.Join(cssDirectory, "test_resources", filename))
		if err != nil { test.Fatal(err) }
		var selectors []struct { Selector string }
		if err := json.Unmarshal(data, &selectors); err != nil { test.Fatal(err) }
		for _, selector := range selectors { addCSSCase(string(cssDocument), selector.Selector) }
	}
	addRegexCase := func(pattern, input string) {
		compiled, err := regexp.Compile(pattern)
		item := rustRegexCase{Pattern: pattern, Input: input, Valid: err == nil}
		if err == nil { item.Matches = compiled.MatchString(input) }
		result.CSSRegex = append(result.CSSRegex, item)
	}
	for scalar := rune(0); scalar <= unicode.MaxRune; scalar++ {
		if folded := unicode.SimpleFold(scalar); folded != scalar { addRegexCase("(?i)"+regexp.QuoteMeta(string(scalar)), string(folded)) }
	}
	for _, pattern := range []string{`\w+`, `\W+`, `\s`, `\d`, `\bword\b`, `\Bword\B`, `[\dA]`, `[a&&b]`, `[a~~b]`, `[[abc]`, `[[:alpha:]]`, `[\b]`, `\Q[raw]+\E`, `\123`, `\u0041`, `(?x)a`, `(?u)a`, `(?P<1>.)`, `(?<n>.)`, `a{1001}`, `a{1000}`, `(a{40}){30}`, `\p{N}`, `\p{Lu}`, `\pL`, `\p{Greek}`, `\P{Greek}`, `\p{White_Space}`} {
		for _, input := range []string{"", "a", "b", "&", "~", "[", "S", "123", "word", " word ", "\v", "\u00a0", "\u017f", "\u0130", "\U00011de0", "\U00010d50", "[raw]+"} { addRegexCase(pattern, input) }
	}
	extractionInputs := append([]string(nil), sequenceInputs...)
	extractionInputs = append(extractionInputs, `<title>Article title</title><link rel="canonical" href="https://example.org/article#part"><meta name="author" content="Writer Name"><meta property="og:locale" content="en_US"><article><h1>Article title</h1><p>`+strings.Repeat("This is an English article with useful information. ", 10)+`</p><p class="bar">Prune this paragraph.</p><p><a href="next">Next link</a></p></article>`, `<title>Title only</title><p>Short text</p>`, `<title>French article</title><meta http-equiv="content-language" content="fr"><p>Bonjour le monde.</p>`)
	for _, input := range extractionInputs {
		for _, focus := range []ExtractionFocus{Balanced, FavorRecall, FavorPrecision} {
			for flags := 0; flags < 64; flags++ {
				for variant := 0; variant < 8; variant++ {
					if variant > 0 && flags != 0 { continue }
					document, _ := html.Parse(strings.NewReader(input))
					before := dom.OuterHTML(document)
					options := Options{Focus: focus, IncludeImages: flags&1 != 0, IncludeLinks: flags&2 != 0, Deduplicate: flags&4 != 0, EnableFallback: flags&8 != 0, ExcludeComments: flags&16 != 0, ExcludeTables: flags&32 != 0, HtmlDateMode: Disabled}
					if variant != 1 { options.OriginalURL, _ = nurl.Parse("https://example.com/news/page") }
					if variant == 1 || variant == 2 { options.HasEssentialMetadata = true }
					if variant == 3 { options.TargetLanguage = "en" }
					if variant == 4 { options.MaxTreeSize = 1 }
					if variant == 5 { options.Config = DefaultConfig(); options.Config.MinOutputSize = 50000; options.Config.MinOutputCommentSize = 50000 }
					if variant == 6 { options.PruneSelector = "p.bar, aside" }
					if variant == 7 { options.PruneSelector = "p:not(" }
					item := rustExtractionCase{HTML: input, Focus: focus, Flags: flags, Variant: variant}
					extracted, err := ExtractDocument(document, options)
					if err != nil { item.Error = err.Error() } else { item.Content = dom.OuterHTML(extracted.ContentNode); item.Comments = dom.OuterHTML(extracted.CommentsNode); item.ContentText = extracted.ContentText; item.CommentsText = extracted.CommentsText; item.Metadata = extracted.Metadata }
					if before != dom.OuterHTML(document) { test.Fatal("extraction changed caller") }
					result.Extraction = append(result.Extraction, item)
				}
			}
		}
	}
	result.ParsedInputs = map[string]any{}
	captureInput := func(input string) {
		if _, exists := result.ParsedInputs[input]; exists { return }
		document, err := html.Parse(strings.NewReader(input))
		if err != nil { test.Fatal(err) }
		result.ParsedInputs[input] = rustParsedInput(document)
	}
	for _, sample := range result.TextNodes { captureInput(sample.HTML) }
	for _, sample := range result.Sequences { captureInput(sample.HTML) }
	for _, sample := range result.Extraction { captureInput(sample.HTML) }
	encoded, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		test.Fatal(err)
	}
	if err := os.WriteFile(os.Getenv("RUST_REFERENCE_OUTPUT"), append(encoded, '\n'), 0644); err != nil {
		test.Fatal(err)
	}
	test.Logf("Exported %d text, %d list, %d DOM, %d metadata, %d date, %d URL cases", len(result.Text), len(result.Lists), len(result.DOM), len(result.Metadata), len(result.Dates), len(result.URLs))
	test.Logf("Added %d URL resolution and %d normalization cases", len(result.URLResolution), len(result.Normalization))
}
