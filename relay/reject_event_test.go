package main

import (
	"context"
	"testing"

	"fiatjaf.com/nostr"
	"fiatjaf.com/nostr/nip29"
	"github.com/puzpuzpuz/xsync/v3"
)

func TestRejectEventAllowsCanonicalEventsWithoutGroupTag(t *testing.T) {
	state := &GroupsState{
		Groups: xsync.NewMapOf[string, *Group](),
	}

	for _, kind := range []nostr.Kind{nostr.KindArticle, nostr.KindHighlights} {
		event := nostr.Event{
			Kind:   kind,
			PubKey: nostr.Generate().Public(),
		}

		reject, msg := state.RejectEvent(context.Background(), event)
		if reject {
			t.Fatalf("RejectEvent() rejected kind %d without h tag: %s", kind, msg)
		}
	}
}

func TestRejectEventStillRequiresGroupTagForGroupScopedEvents(t *testing.T) {
	state := &GroupsState{
		Groups: xsync.NewMapOf[string, *Group](),
	}

	event := nostr.Event{
		Kind:   11,
		PubKey: nostr.Generate().Public(),
	}

	reject, msg := state.RejectEvent(context.Background(), event)
	if !reject {
		t.Fatalf("RejectEvent() accepted kind %d without h tag", event.Kind)
	}
	if msg != "missing group (`h`) tag" {
		t.Fatalf("RejectEvent() message = %q, want %q", msg, "missing group (`h`) tag")
	}
}

func TestHideEventFromReaderAllowsCanonicalUngroupedEvents(t *testing.T) {
	state := &GroupsState{
		Groups: xsync.NewMapOf[string, *Group](),
	}

	for _, kind := range []nostr.Kind{nostr.KindArticle, nostr.KindHighlights} {
		if hide := state.hideEventFromReader(nostr.Event{Kind: kind}, nil); hide {
			t.Fatalf("hideEventFromReader() hid ungrouped kind %d", kind)
		}
	}
}

func TestHideEventFromReaderBlocksPrivateGroupEventsForNonMembers(t *testing.T) {
	state := &GroupsState{
		Groups: xsync.NewMapOf[string, *Group](),
	}

	member := nostr.Generate().Public()
	group := &Group{
		Group: nip29.Group{
			Address: nip29.GroupAddress{ID: "private-group"},
			Private: true,
			Members: map[nostr.PubKey][]*nip29.Role{
				member: nil,
			},
		},
	}
	state.Groups.Store(group.Address.ID, group)

	event := nostr.Event{
		Kind: 11,
		Tags: nostr.Tags{
			{"h", group.Address.ID},
		},
	}

	if hide := state.hideEventFromReader(event, nil); !hide {
		t.Fatalf("hideEventFromReader() exposed a private-group event to a non-member")
	}
	if hide := state.hideEventFromReader(event, []nostr.PubKey{member}); hide {
		t.Fatalf("hideEventFromReader() hid a private-group event from a member")
	}
}
