import { useLingui } from "@lingui/react/macro";
import type { EditorControl, SlashTriggerMatch } from "@swarmnote/editor-core";
import { CharTriggerPopover } from "@/components/editor/CharTriggerPopover";

interface SlashCommandPopoverProps {
  match: SlashTriggerMatch | null;
  control: EditorControl | null;
}

export function SlashCommandPopover({ match, control }: SlashCommandPopoverProps) {
  const { t } = useLingui();
  return (
    <CharTriggerPopover
      match={match}
      control={control}
      commandPrefix="slash"
      emptyLabel={t`No matching commands`}
    />
  );
}
