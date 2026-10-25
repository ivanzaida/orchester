/**
 * A route handler's `303 See Other` to `path` on the host the request came to: a relative `Location`, so a visitor on
 * any of the site's origins (`Config.siteOrigins`) stays on it, with the host-only cookies that host set. The browser
 * follows with a `GET`, never posting the form a second time.
 */
export function seeOther(path: string): Response {
  return new Response(null, { status: 303, headers: { location: path } });
}
