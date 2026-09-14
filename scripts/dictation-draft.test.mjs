import assert from 'node:assert/strict'
import { DictationDraft, joinTranscript } from '../apps/web/src/lib/dictation-draft.ts'
const draft = new DictationDraft('Before after', 7, true)
assert.deepEqual(draft.accept('hello', false), { text: 'Before hello after', caret: 12 })
assert.equal(draft.accept('hello world.', false).text, 'Before hello world. after')
assert.equal(draft.accept('hello world.', true).text, 'Before hello world. after')
assert.equal(draft.accept('world. Another sentence.', true).text, 'Before hello world. Another sentence. after')
assert.equal(new DictationDraft('Existing', 8, false).accept(' Hello, world!', true).text, 'Existing Hello world')
assert.equal(new DictationDraft('Existing text', 8, true).accept('', true).text, 'Existing text')
assert.equal(joinTranscript('I can hear you.', 'you. Can you hear me?'), 'I can hear you. Can you hear me?')
console.log('PASS partial replacement, committed overlap, caret insertion, preserved existing text and punctuation')
