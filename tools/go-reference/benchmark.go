package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net/url"
	"os"
	"strings"
	"time"

	"github.com/go-shiori/dom"
	trafilatura "github.com/markusmobius/go-trafilatura/v2"
	"golang.org/x/net/html"
)

type benchmarkPage struct {
	File, URL, HTML string
	With, Without   []string
}

type benchmarkRequest struct {
	Fallback       bool   `json:"fallback"`
	Outputs        bool   `json:"outputs"`
	InputTree      bool   `json:"input_tree"`
	Focus          string `json:"focus"`
	Comments       bool   `json:"comments"`
	Images         bool   `json:"images"`
	Links          bool   `json:"links"`
	ExcludeTables  bool   `json:"exclude_tables"`
	Deduplicate    bool   `json:"deduplicate"`
	TargetLanguage string `json:"target_language"`
}

type benchmarkCounts struct {
	TruePositives  int `json:"true_positives"`
	FalseNegatives int `json:"false_negatives"`
	FalsePositives int `json:"false_positives"`
	TrueNegatives  int `json:"true_negatives"`
}

func (counts *benchmarkCounts) evaluate(text string, page benchmarkPage) {
	for _, snippet := range page.With {
		if text != "" && strings.Contains(text, snippet) {
			counts.TruePositives++
		} else {
			counts.FalseNegatives++
		}
	}
	for _, snippet := range page.Without {
		if text != "" && strings.Contains(text, snippet) {
			counts.FalsePositives++
		} else {
			counts.TrueNegatives++
		}
	}
}

type benchmarkOutput struct {
	File             string                `json:"file"`
	InputTree        any                   `json:"input_tree,omitempty"`
	Text             string                `json:"text"`
	Comments         string                `json:"comments"`
	SelectedText     string                `json:"selected_text"`
	SelectedComments string                `json:"selected_comments"`
	HTML             string                `json:"html"`
	CommentsHTML     string                `json:"comments_html"`
	Metadata         *trafilatura.Metadata `json:"metadata"`
	Error            string                `json:"error"`
}

type benchmarkError struct {
	File  string `json:"file"`
	Error string `json:"error"`
}

func inputSnapshot(node *html.Node) any {
	if node.Type == html.DocumentNode {
		return inputSnapshot(dom.QuerySelector(node, "html"))
	}
	if node.Type == html.CommentNode {
		return map[string]any{"comment": node.Data}
	}
	if node.Type == html.TextNode {
		return node.Data
	}
	attributes := [][2]string{}
	for _, attribute := range node.Attr {
		key := attribute.Key
		if attribute.Namespace != "" {
			key = attribute.Namespace + ":" + key
		}
		attributes = append(attributes, [2]string{key, attribute.Val})
	}
	children := []any{}
	for child := node.FirstChild; child != nil; child = child.NextSibling {
		if child.Type == html.ElementNode || child.Type == html.CommentNode || child.Type == html.TextNode {
			children = append(children, inputSnapshot(child))
		}
	}
	return map[string]any{"tag": node.Data, "attributes": attributes, "children": children}
}

type benchmarkResponse struct {
	ElapsedNS int64             `json:"elapsed_ns"`
	Counts    benchmarkCounts   `json:"counts"`
	Errors    []benchmarkError  `json:"errors"`
	Outputs   []benchmarkOutput `json:"outputs"`
}

func runPageBenchmark() error {
	flags := flag.NewFlagSet("jsonl", flag.ContinueOnError)
	fallback := flags.Bool("fallback", false, "Enable native fallbacks")
	comments := flags.Bool("comments", false, "Extract comments")
	inputTree := flags.Bool("input-tree", false, "Include parsed input diagnostics")
	focus := flags.String("focus", "balanced", "balanced, precision, or recall")
	if err := flags.Parse(os.Args[2:]); err != nil {
		return err
	}
	options := trafilatura.Options{EnableFallback: *fallback, ExcludeComments: !*comments}
	switch *focus {
	case "balanced":
	case "precision":
		options.Focus = trafilatura.FavorPrecision
	case "recall":
		options.Focus = trafilatura.FavorRecall
	default:
		return fmt.Errorf("unknown focus: %s", *focus)
	}
	decoder := json.NewDecoder(os.Stdin)
	encoder := json.NewEncoder(os.Stdout)
	for {
		var input struct {
			ID       string `json:"id"`
			URL      string `json:"url"`
			HTMLPath string `json:"html_path"`
		}
		if err := decoder.Decode(&input); err == io.EOF {
			return nil
		} else if err != nil {
			return err
		}
		if input.ID == "" || input.HTMLPath == "" {
			return fmt.Errorf("each input requires id and html_path")
		}
		output := benchmarkOutput{File: input.ID}
		options.OriginalURL, _ = url.ParseRequestURI(input.URL)
		file, err := os.Open(input.HTMLPath)
		var document *html.Node
		if err == nil {
			document, err = dom.Parse(file)
			file.Close()
		}
		if err == nil && *inputTree {
			output.InputTree = inputSnapshot(document)
		}
		var result *trafilatura.ExtractResult
		if err == nil {
			result, err = trafilatura.ExtractDocument(document, options)
		}
		if err != nil {
			output.Error = err.Error()
		} else {
			output.Text, output.Comments = result.ContentText, result.CommentsText
			output.SelectedText = dom.TextContent(result.ContentNode)
			output.HTML = dom.OuterHTML(result.ContentNode)
			if result.CommentsNode != nil {
				output.SelectedComments = dom.TextContent(result.CommentsNode)
				output.CommentsHTML = dom.OuterHTML(result.CommentsNode)
			}
			output.Metadata = &result.Metadata
		}
		if err := encoder.Encode(output); err != nil {
			return err
		}
	}
}

func runBenchmark() error {
	if len(os.Args) > 1 && os.Args[1] == "--jsonl" {
		return runPageBenchmark()
	}
	if len(os.Args) != 2 {
		return fmt.Errorf("usage: benchmark CORPUS_JSON")
	}
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		return err
	}
	var pages []benchmarkPage
	if err := json.Unmarshal(data, &pages); err != nil {
		return err
	}
	documents := make([]*html.Node, len(pages))
	for index, page := range pages {
		documents[index], err = html.Parse(strings.NewReader(strings.TrimPrefix(page.HTML, "\ufeff")))
		if err != nil {
			return err
		}
	}
	encoder := json.NewEncoder(os.Stdout)
	if err := encoder.Encode(map[string]int{"ready": len(pages)}); err != nil {
		return err
	}
	decoder := json.NewDecoder(os.Stdin)
	for {
		var request benchmarkRequest
		if err := decoder.Decode(&request); err == io.EOF {
			return nil
		} else if err != nil {
			return err
		}
		focus := trafilatura.Balanced
		switch request.Focus {
		case "", "balanced":
		case "precision":
			focus = trafilatura.FavorPrecision
		case "recall":
			focus = trafilatura.FavorRecall
		default:
			return fmt.Errorf("unknown focus: %s", request.Focus)
		}
		configured := make([]trafilatura.Options, len(pages))
		for index, page := range pages {
			originalURL, _ := url.ParseRequestURI(page.URL)
			configured[index] = trafilatura.Options{
				OriginalURL: originalURL, EnableFallback: request.Fallback,
				Focus: focus, ExcludeComments: !request.Comments, ExcludeTables: request.ExcludeTables,
				IncludeImages: request.Images, IncludeLinks: request.Links, Deduplicate: request.Deduplicate,
				TargetLanguage: request.TargetLanguage,
			}
		}
		response := benchmarkResponse{Errors: []benchmarkError{}, Outputs: []benchmarkOutput{}}
		started := time.Now()
		for index, page := range pages {
			result, err := trafilatura.ExtractDocument(documents[index], configured[index])
			output := benchmarkOutput{File: page.File}
			if err != nil {
				response.Counts.evaluate("", page)
				response.Errors = append(response.Errors, benchmarkError{File: page.File, Error: err.Error()})
				output.Error = err.Error()
			} else {
				response.Counts.evaluate(result.ContentText, page)
				if request.Outputs {
					output.Text, output.Comments = result.ContentText, result.CommentsText
					output.SelectedText = dom.TextContent(result.ContentNode)
					output.HTML = dom.OuterHTML(result.ContentNode)
					if result.CommentsNode != nil {
						output.SelectedComments = dom.TextContent(result.CommentsNode)
						output.CommentsHTML = dom.OuterHTML(result.CommentsNode)
					}
					output.Metadata = &result.Metadata
				}
			}
			if request.Outputs {
				if request.InputTree {
					output.InputTree = inputSnapshot(documents[index])
				}
				response.Outputs = append(response.Outputs, output)
			}
		}
		response.ElapsedNS = time.Since(started).Nanoseconds()
		if err := encoder.Encode(response); err != nil {
			return err
		}
	}
}

func main() {
	if err := runBenchmark(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
