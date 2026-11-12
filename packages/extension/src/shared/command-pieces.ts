/**
 * The command split into the pieces a line may end after: each `/` and each
 * space (so a `--flag` starts a line). Words and host names stay whole.
 */
export function commandPieces(command: string): string[] {
  return command.match(/[^/\s]*(?:\/+|\s+|$)/g)?.filter((piece) => piece !== "") ?? [command];
}
