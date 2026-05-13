import { useLingui } from "@lingui/react/macro";
import type { EditorControl, WikilinkTriggerMatch } from "@swarmnote/editor-core";
import { CharTriggerPopover } from "@/components/editor/CharTriggerPopover";

interface WikilinkPopoverProps {
  match: WikilinkTriggerMatch | null;
  control: EditorControl | null;
}

export function WikilinkPopover({ match, control }: WikilinkPopoverProps) {
  const { t } = useLingui();
  return (
    <CharTriggerPopover
      match={match}
      control={control}
      commandPrefix="wikilink"
      headerLabel={t`Link to note`}
      emptyLabel={t`No matching notes`}
    />
  );
}
