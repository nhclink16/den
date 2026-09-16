/// A reactive cell the Store owns. Logic classes take one of these instead of
/// declaring runes themselves, so the same code the app runs can be exercised
/// without a component runtime.
export type Box<T> = { get(): T; set(value: T): void }
