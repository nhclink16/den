function validatePickerChoice(offeredIds, sourceId) {
  if (sourceId === null || sourceId === undefined) return null
  if (!offeredIds.has(sourceId)) return false
  return sourceId
}

function validatePickerFrame(frame, mainFrame, trusted) {
  return !!(frame && frame === mainFrame && trusted(frame.url))
}

module.exports = { validatePickerChoice, validatePickerFrame }
