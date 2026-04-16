
// this file is generated — do not edit it


declare module "svelte/elements" {
	export interface HTMLAttributes<T> {
		'data-sveltekit-keepfocus'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-noscroll'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-preload-code'?:
			| true
			| ''
			| 'eager'
			| 'viewport'
			| 'hover'
			| 'tap'
			| 'off'
			| undefined
			| null;
		'data-sveltekit-preload-data'?: true | '' | 'hover' | 'tap' | 'off' | undefined | null;
		'data-sveltekit-reload'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-replacestate'?: true | '' | 'off' | undefined | null;
	}
}

export {};


declare module "$app/types" {
	type MatcherParam<M> = M extends (param : string) => param is (infer U extends string) ? U : string;

	export interface AppTypes {
		RouteId(): "/" | "/.well-known" | "/.well-known/nostr.json" | "/about" | "/api" | "/api/debug" | "/api/debug/front-page-cache" | "/api/nip05" | "/bookmarks" | "/community" | "/community/create" | "/community/[id]" | "/community/[id]/content" | "/community/[id]/content/[contentId]" | "/community/[id]/content/[contentId]/highlight" | "/community/[id]/content/[contentId]/highlight/[highlightId]" | "/highlights" | "/me" | "/note" | "/note/[id]" | "/og" | "/og/note" | "/og/note/[id]" | "/onboarding" | "/profile" | "/profile/edit" | "/profile/[identifier]" | "/relays" | "/relay" | "/relay/[hostname]" | "/share" | "/share/community" | "/share/community/[id]" | "/share/highlight" | "/share/highlight/[id]";
		RouteParams(): {
			"/community/[id]": { id: string };
			"/community/[id]/content": { id: string };
			"/community/[id]/content/[contentId]": { id: string; contentId: string };
			"/community/[id]/content/[contentId]/highlight": { id: string; contentId: string };
			"/community/[id]/content/[contentId]/highlight/[highlightId]": { id: string; contentId: string; highlightId: string };
			"/note/[id]": { id: string };
			"/og/note/[id]": { id: string };
			"/profile/[identifier]": { identifier: string };
			"/relay/[hostname]": { hostname: string };
			"/share/community/[id]": { id: string };
			"/share/highlight/[id]": { id: string }
		};
		LayoutParams(): {
			"/": { id?: string; contentId?: string; highlightId?: string; identifier?: string; hostname?: string };
			"/.well-known": Record<string, never>;
			"/.well-known/nostr.json": Record<string, never>;
			"/about": Record<string, never>;
			"/api": Record<string, never>;
			"/api/debug": Record<string, never>;
			"/api/debug/front-page-cache": Record<string, never>;
			"/api/nip05": Record<string, never>;
			"/bookmarks": Record<string, never>;
			"/community": { id?: string; contentId?: string; highlightId?: string };
			"/community/create": Record<string, never>;
			"/community/[id]": { id: string; contentId?: string; highlightId?: string };
			"/community/[id]/content": { id: string; contentId?: string; highlightId?: string };
			"/community/[id]/content/[contentId]": { id: string; contentId: string; highlightId?: string };
			"/community/[id]/content/[contentId]/highlight": { id: string; contentId: string; highlightId?: string };
			"/community/[id]/content/[contentId]/highlight/[highlightId]": { id: string; contentId: string; highlightId: string };
			"/highlights": Record<string, never>;
			"/me": Record<string, never>;
			"/note": { id?: string };
			"/note/[id]": { id: string };
			"/og": { id?: string };
			"/og/note": { id?: string };
			"/og/note/[id]": { id: string };
			"/onboarding": Record<string, never>;
			"/profile": { identifier?: string };
			"/profile/edit": Record<string, never>;
			"/profile/[identifier]": { identifier: string };
			"/relays": Record<string, never>;
			"/relay": { hostname?: string };
			"/relay/[hostname]": { hostname: string };
			"/share": { id?: string };
			"/share/community": { id?: string };
			"/share/community/[id]": { id: string };
			"/share/highlight": { id?: string };
			"/share/highlight/[id]": { id: string }
		};
		Pathname(): "/" | "/.well-known/nostr.json" | "/about" | "/api/debug/front-page-cache" | "/api/nip05" | "/bookmarks" | "/highlights" | `/note/${string}` & {} | `/og/note/${string}` & {} | "/onboarding" | "/profile/edit" | `/profile/${string}` & {} | "/relays" | `/relay/${string}` & {};
		ResolvedPathname(): `${"" | `/${string}`}${ReturnType<AppTypes['Pathname']>}`;
		Asset(): "/og-default.png" | string & {};
	}
}