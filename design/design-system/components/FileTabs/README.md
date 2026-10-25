# FileTabs

One tab per loaded log. Each shows the ECU source as a three-letter mono `ecu` badge (HAL Haltech, LNK Link, AIM AiM, RUS rusEFI/Speeduino, EMU ECUMaster, MS MegaSquirt…), the file name truncated in the middle so the extension stays visible, and a close button.

Active tab: `bg-raised`, `ink`, `line-control` border, a 2px `cyan` inset on the top edge. Inactive: `bg-panel`, `ink-muted`. Full file name in the tooltip. Consumer provides: file name, ECU code, active state, close handler.
