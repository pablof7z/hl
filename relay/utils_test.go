package main

import "testing"

func TestFilepathJoinWithAbsoluteJoinsRelativePaths(t *testing.T) {
	got := filepathJoinWithAbsolute("/tmp/highlighter/data", "blossom-files")
	want := "/tmp/highlighter/data/blossom-files"

	if got != want {
		t.Fatalf("filepathJoinWithAbsolute() = %q, want %q", got, want)
	}
}

func TestFilepathJoinWithAbsoluteRespectsAbsolutePaths(t *testing.T) {
	got := filepathJoinWithAbsolute("/tmp/highlighter/data", "/var/lib/blossom")
	want := "/var/lib/blossom"

	if got != want {
		t.Fatalf("filepathJoinWithAbsolute() = %q, want %q", got, want)
	}
}
