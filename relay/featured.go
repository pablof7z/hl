package main

import (
	"net/http"
	"sort"
	"strings"

	"fiatjaf.com/croissant/global"
	"fiatjaf.com/nostr"
)

// searchKey produces the lowercase concatenation of fields the admin picker's
// client-side filter matches against.
func searchKey(g orderedGroup) string {
	return strings.ToLower(g.Name + "\n" + g.ID + "\n" + g.About)
}

// kindFeaturedRooms is the NIP-51 parameterless list the relay publishes to
// advertise the editorial "Featured" shelf in the iOS rooms explorer. One
// `group` tag per featured room, in the curator-chosen order.
const kindFeaturedRooms = nostr.Kind(10012)

// orderedGroup is a snapshot of a group needed by the picker — lifted out of
// the xsync map so the templ template can render them in a stable order.
type orderedGroup struct {
	ID      string
	Name    string
	About   string
	Picture string
	Hidden  bool
}

func snapshotGroups() []orderedGroup {
	out := make([]orderedGroup, 0, State.Groups.Size())
	for id, group := range State.Groups.Range {
		if group.Hidden {
			continue
		}
		out = append(out, orderedGroup{
			ID:      id,
			Name:    group.Name,
			About:   group.About,
			Picture: group.Picture,
		})
	}
	sort.Slice(out, func(i, j int) bool {
		// Groups without a name sort by id so they're still deterministic.
		ni := strings.ToLower(out[i].Name)
		nj := strings.ToLower(out[j].Name)
		if ni == nj {
			return out[i].ID < out[j].ID
		}
		if ni == "" {
			return false
		}
		if nj == "" {
			return true
		}
		return ni < nj
	})
	return out
}

// currentFeaturedSet reads the relay's newest kind:10012 from the local store
// and returns the set of group ids it tags. Empty map on first run (no list
// has been published yet).
func currentFeaturedSet() map[string]bool {
	out := map[string]bool{}
	for evt := range State.DB.QueryEvents(nostr.Filter{
		Kinds:   []nostr.Kind{kindFeaturedRooms},
		Authors: []nostr.PubKey{global.S.RelayPublicKey()},
	}, 1) {
		for _, tag := range evt.Tags {
			if len(tag) >= 2 && tag[0] == "group" {
				out[tag[1]] = true
			}
		}
	}
	return out
}

// featuredAdminHandler renders the admin picker for the featured-rooms list.
// Admin-gated by OwnerPubKey so only the relay operator can change the shelf.
func featuredAdminHandler(w http.ResponseWriter, r *http.Request) {
	loggedUser := global.LoggedUserFromContext(r.Context())
	if loggedUser != global.S.OwnerPubKey {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	if err := featuredAdmin(snapshotGroups(), currentFeaturedSet()).Render(r.Context(), w); err != nil {
		L.Error().Err(err).Msg("failed to render featured admin")
	}
}

// publishFeaturedHandler parses the admin form, builds a fresh kind:10012
// signed with the relay's secret key, and replaces the existing list. The
// event is also broadcast on the relay's pool so any live iOS client picks
// it up immediately.
func publishFeaturedHandler(w http.ResponseWriter, r *http.Request) {
	loggedUser := global.LoggedUserFromContext(r.Context())
	if loggedUser != global.S.OwnerPubKey {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	if err := r.ParseForm(); err != nil {
		http.Error(w, "invalid form", http.StatusBadRequest)
		return
	}

	selected := r.Form["featured"]
	seen := map[string]bool{}
	tags := make(nostr.Tags, 0, len(selected))
	relayURL := global.S.RelayWSURL()
	for _, id := range selected {
		id = strings.TrimSpace(id)
		if id == "" || seen[id] {
			continue
		}
		// Accept only ids that correspond to a currently-known group — a stray
		// form value from a stale page shouldn't end up in the published list.
		if _, ok := State.Groups.Load(id); !ok {
			continue
		}
		seen[id] = true
		tags = append(tags, nostr.Tag{"group", id, relayURL})
	}

	event := nostr.Event{
		Kind:      kindFeaturedRooms,
		CreatedAt: nostr.Now(),
		Tags:      tags,
		Content:   "",
	}
	if err := event.Sign(global.S.RelaySecretKey); err != nil {
		L.Error().Err(err).Msg("sign featured list")
		http.Error(w, "failed to sign featured list", http.StatusInternalServerError)
		return
	}
	if err := State.DB.ReplaceEvent(event); err != nil {
		L.Error().Err(err).Msg("save featured list")
		http.Error(w, "failed to save featured list", http.StatusInternalServerError)
		return
	}
	global.R.BroadcastEvent(event)

	http.Redirect(w, r, "/admin/featured", http.StatusSeeOther)
}
