/** Index 0 is the individual mode, index 1 the continuous one. */
export const routes: RegExp[] = [/^\/$/, /^\/continuous$/];

/** The route index for a path; anything unknown falls back to 個別. */
export function matchRoute(pathname: string): number {
  const index = routes.findIndex((pattern) => pattern.test(pathname));
  return index === -1 ? 0 : index;
}
