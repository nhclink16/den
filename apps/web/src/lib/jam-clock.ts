// Where the Jam's progress clock is anchored. The server caches a sample for a few
// seconds and keeps handing the same one out, and its clock may disagree with
// this browser's, so only the difference between two server stamps is trusted.
export type Sample = { track: string; sampled_at: number; progress_ms: number }
export type Anchor = { track: string; sampled: number; at: number; ms: number }

/** The local moment `ms` of progress was true, given the previous anchor and a new sample. */
export function anchor(prev: Anchor | null, sample: Sample, now: number): Anchor {
  if (prev && prev.track === sample.track) {
    // The same cached sample again: nothing new, so the running clock stands.
    if (sample.sampled_at === prev.sampled) return prev
    // A newer sample of the same track: place it where it belongs on our clock,
    // using the server-side gap between the two samples rather than "now".
    if (sample.sampled_at > prev.sampled) {
      return { track: sample.track, sampled: sample.sampled_at, at: prev.at + (sample.sampled_at - prev.sampled), ms: sample.progress_ms }
    }
  }
  return { track: sample.track, sampled: sample.sampled_at, at: now, ms: sample.progress_ms }
}
