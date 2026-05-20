export function isSelectionInsideCode(state) {
  const { $from } = state.selection;
  return $from.marks().some((mark) => mark.type.spec?.code) || $from.parent.type.spec?.code === true;
}
