package global

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"fiatjaf.com/nostr"
)

// TestLoadSettingsWithZeroPubKey reproduces the bug where a settings.json
// containing a zero owner_pubkey (written on first run before OWNER_PUBLIC_KEY
// env var is set) causes loadSettings to fail with a curve validation error.
func TestLoadSettingsWithZeroPubKey(t *testing.T) {
	dir := t.TempDir()

	// Simulate what happens on second run: settings.json exists with zero pubkey
	initial := map[string]interface{}{
		"relay_name":        "croissant",
		"relay_description": "groups provider",
		"relay_secret_key":  "fd462fdc47769ae08dd0bae5b4fe36f8c64f32467da6af5499eabc186449d563",
		"owner_pubkey":      "0000000000000000000000000000000000000000000000000000000000000000",
		"blossom": map[string]interface{}{
			"enabled":    false,
			"local_path": "blossom-files",
		},
		"groups": map[string]interface{}{
			"create_group_rate_limit": map[string]interface{}{
				"tokens_per_interval": 1,
				"interval_seconds":    10800,
				"max_tokens":          3,
			},
		},
	}

	data, err := json.MarshalIndent(initial, "", "  ")
	if err != nil {
		t.Fatalf("failed to marshal test settings: %v", err)
	}

	settingsFile := filepath.Join(dir, "settings.json")
	if err := os.WriteFile(settingsFile, data, 0644); err != nil {
		t.Fatalf("failed to write test settings file: %v", err)
	}

	// This should NOT fail — zero pubkey in JSON is valid (means "unset")
	settings, err := loadSettings(dir)
	if err != nil {
		t.Fatalf("loadSettings failed with zero owner_pubkey: %v", err)
	}

	// OwnerPubKey should be zero (init.go will override it from env)
	if settings.OwnerPubKey != nostr.ZeroPK {
		t.Errorf("expected zero OwnerPubKey, got %s", settings.OwnerPubKey.Hex())
	}

	// RelaySecretKey should be loaded correctly
	expectedSK, _ := nostr.SecretKeyFromHex("fd462fdc47769ae08dd0bae5b4fe36f8c64f32467da6af5499eabc186449d563")
	if settings.RelaySecretKey != expectedSK {
		t.Errorf("RelaySecretKey not loaded correctly")
	}
}

// TestLoadSettingsWithValidPubKey ensures a valid owner_pubkey is preserved.
func TestLoadSettingsWithValidPubKey(t *testing.T) {
	dir := t.TempDir()

	validPubKey := "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52"

	initial := map[string]interface{}{
		"relay_name":       "test relay",
		"relay_secret_key": "fd462fdc47769ae08dd0bae5b4fe36f8c64f32467da6af5499eabc186449d563",
		"owner_pubkey":     validPubKey,
		"blossom":          map[string]interface{}{"local_path": "blossom-files"},
		"groups": map[string]interface{}{
			"create_group_rate_limit": map[string]interface{}{
				"tokens_per_interval": 1,
				"interval_seconds":    10800,
				"max_tokens":          3,
			},
		},
	}

	data, _ := json.MarshalIndent(initial, "", "  ")
	settingsFile := filepath.Join(dir, "settings.json")
	if err := os.WriteFile(settingsFile, data, 0644); err != nil {
		t.Fatalf("failed to write test settings file: %v", err)
	}

	settings, err := loadSettings(dir)
	if err != nil {
		t.Fatalf("loadSettings failed with valid owner_pubkey: %v", err)
	}

	expectedPK, err := nostr.PubKeyFromHex(validPubKey)
	if err != nil {
		t.Fatalf("failed to parse expected pubkey: %v", err)
	}

	if settings.OwnerPubKey != expectedPK {
		t.Errorf("expected OwnerPubKey %s, got %s", validPubKey, settings.OwnerPubKey.Hex())
	}
}

// TestLoadSettingsNoFile verifies fresh initialization works without a
// settings.json, and the resulting file can be loaded on the next call.
func TestLoadSettingsNoFile(t *testing.T) {
	dir := t.TempDir()

	// First call — creates settings.json
	_, err := loadSettings(dir)
	if err != nil {
		t.Fatalf("first loadSettings call failed: %v", err)
	}

	// Second call — loads the just-created settings.json (previously this failed
	// because the saved file contained a zero owner_pubkey)
	_, err = loadSettings(dir)
	if err != nil {
		t.Fatalf("second loadSettings call failed (zero pubkey regression): %v", err)
	}
}
