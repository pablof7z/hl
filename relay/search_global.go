package main

import (
	"iter"
	"path/filepath"
	"slices"

	"fiatjaf.com/croissant/global"
	"fiatjaf.com/nostr"
)

const globalSearchIndexID = "_global"

var globalIndexableKinds = []nostr.Kind{
	nostr.KindArticle,
	nostr.KindSimpleGroupMetadata,
}

func (s *GroupsState) IndexEvent(event nostr.Event) error {
	if group := s.GetGroupFromEvent(event); group != nil {
		if err := group.IndexEvent(event); err != nil {
			return err
		}
	}

	if !slices.Contains(globalIndexableKinds, event.Kind) {
		return nil
	}

	index, err := s.ensureGlobalSearchIndex()
	if err != nil {
		return err
	}
	if index == nil {
		return nil
	}

	return index.SaveEvent(event)
}

func (s *GroupsState) DeindexEvent(id nostr.ID) error {
	if s.globalSearchIndex != nil {
		return s.globalSearchIndex.DeleteEvent(id)
	}

	return nil
}

func (s *GroupsState) SearchGlobalEvents(filter nostr.Filter, maxLimit int) iter.Seq[nostr.Event] {
	requestedKinds := len(filter.Kinds) > 0
	if requestedKinds {
		for i := 0; i < len(filter.Kinds); i++ {
			if !slices.Contains(globalIndexableKinds, filter.Kinds[i]) {
				filter.Kinds[i] = filter.Kinds[len(filter.Kinds)-1]
				filter.Kinds = filter.Kinds[:len(filter.Kinds)-1]
				i--
			}
		}

		if len(filter.Kinds) == 0 {
			return func(yield func(nostr.Event) bool) {}
		}
	} else {
		filter.Kinds = slices.Clone(globalIndexableKinds)
	}

	index, _ := s.ensureGlobalSearchIndex()
	if index != nil {
		return index.QueryEvents(filter, maxLimit)
	}

	return func(yield func(nostr.Event) bool) {}
}

func (s *GroupsState) ensureGlobalSearchIndex() (*BleveIndex, error) {
	if s.globalSearchIndex != nil {
		return s.globalSearchIndex, nil
	}

	indexPath := filepath.Join(global.E.DataPath, "search", globalSearchIndexID)
	langCode, ok, err := readLanguage(indexPath)
	if err != nil {
		return nil, err
	}
	if ok {
		s.globalSearchIndex = &BleveIndex{
			Path:          indexPath,
			Language:      langCode,
			RawEventStore: store,
		}
		if err := s.globalSearchIndex.Init(); err != nil {
			return nil, err
		}

		return s.globalSearchIndex, nil
	}

	count, err := store.CountEvents(nostr.Filter{Kinds: globalIndexableKinds})
	if err != nil {
		return nil, err
	}
	if count == 0 {
		return nil, nil
	}

	events := collectSearchableEvents(nostr.Filter{Kinds: globalIndexableKinds}, count)
	s.globalSearchIndex = &BleveIndex{
		Path:          indexPath,
		Language:      "simple",
		RawEventStore: store,
	}
	if err := s.globalSearchIndex.Init(); err != nil {
		return nil, err
	}
	if err := writeLanguage(indexPath, "simple"); err != nil {
		s.globalSearchIndex.Close()
		return nil, err
	}

	for _, evt := range events {
		if err := s.globalSearchIndex.SaveEvent(evt); err != nil {
			L.Warn().Err(err).Str("scope", "global").Str("event", evt.ID.Hex()).Msg("failed to index event")
		}
	}

	return s.globalSearchIndex, nil
}
