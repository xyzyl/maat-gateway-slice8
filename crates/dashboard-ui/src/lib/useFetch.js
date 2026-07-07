// Tiny data-fetching hook. Returns { data, error, loading, reload }.
// Designed for the dashboard's pattern: page loads → fetch list → render.
//
// Not a replacement for swr/react-query. We don't need request
// deduplication or background revalidation; the dashboard is operator-
// driven and each page reload from the user's perspective is a fresh
// request anyway.
import { useCallback, useEffect, useState } from "react";
export function useFetch(fetcher, deps = []) {
    const [data, setData] = useState(null);
    const [error, setError] = useState(null);
    const [loading, setLoading] = useState(true);
    const [tick, setTick] = useState(0);
    const reload = useCallback(() => setTick((t) => t + 1), []);
    useEffect(() => {
        let cancelled = false;
        setLoading(true);
        setError(null);
        fetcher()
            .then((result) => {
            if (!cancelled) {
                setData(result);
                setLoading(false);
            }
        })
            .catch((err) => {
            if (!cancelled) {
                setError(err instanceof Error ? err : new Error(String(err)));
                setLoading(false);
            }
        });
        return () => {
            cancelled = true;
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [...deps, tick]);
    return { data, error, loading, reload };
}
