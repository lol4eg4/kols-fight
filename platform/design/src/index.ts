// @arena/design: the only source of colours, type, buttons and KOL Arena pieces for every screen.
// Pieces show only what the caller passes. They never call a route and never import a plugin.
// Styles load once, from the shell: import "@arena/design/styles.css".

export { cx } from "./cx";
export * from "./format";
export { avatarSpec, avatarPath, hashId, AVATAR_COLORS } from "./avatar";
export type { AvatarSpec, AvatarColor } from "./avatar";

export { Button, buttonClass } from "./components/Button";
export type { ButtonProps, ButtonVariant } from "./components/Button";
export { Field } from "./components/Field";
export type { FieldProps } from "./components/Field";
export { Select } from "./components/Select";
export type { SelectProps, SelectOption } from "./components/Select";
export { List, ListItem } from "./components/List";
export type { ListProps, ListItemProps } from "./components/List";
export { Card, StatTile } from "./components/Card";
export type { CardProps, CardTone, StatTileProps } from "./components/Card";
export { Dialog } from "./components/Dialog";
export type { DialogProps } from "./components/Dialog";
export { Tabs, TabPanel } from "./components/Tabs";
export type { TabsProps, TabItem, TabPanelProps } from "./components/Tabs";
export { Badge } from "./components/Badge";
export type { BadgeProps, BadgeTone } from "./components/Badge";
export { Progress } from "./components/Progress";
export type { ProgressProps, ProgressTone } from "./components/Progress";
export { Tooltip } from "./components/Tooltip";
export type { TooltipProps } from "./components/Tooltip";
export { Skeleton } from "./components/Skeleton";
export type { SkeletonProps } from "./components/Skeleton";
export { EmptyState } from "./components/EmptyState";
export type { EmptyStateProps } from "./components/EmptyState";
export { ErrorLine } from "./components/ErrorLine";
export type { ErrorLineProps } from "./components/ErrorLine";
export { Container } from "./components/Container";
export type { ContainerProps } from "./components/Container";
export { Heading } from "./components/Heading";
export type { HeadingProps } from "./components/Heading";
export { Logo } from "./components/Logo";
export type { LogoProps } from "./components/Logo";

export { PixelAvatar } from "./components/PixelAvatar";
export type { PixelAvatarProps } from "./components/PixelAvatar";
export { TierBadge } from "./components/TierBadge";
export type { TierBadgeProps, Tier } from "./components/TierBadge";
export { ScoreBar } from "./components/ScoreBar";
export type { ScoreBarProps } from "./components/ScoreBar";
export { NoData } from "./components/NoData";
export type { NoDataProps } from "./components/NoData";
export { Money } from "./components/Money";
export type { MoneyProps } from "./components/Money";
export { TimeAgo } from "./components/TimeAgo";
export type { TimeAgoProps } from "./components/TimeAgo";
export { SnapshotDate } from "./components/SnapshotDate";
export type { SnapshotDateProps } from "./components/SnapshotDate";
export { ScoreBreakdown } from "./components/ScoreBreakdown";
export type { ScoreBreakdownProps, ScoreBreakdownLabels, ScorePart } from "./components/ScoreBreakdown";
export { Ticker } from "./components/Ticker";
export type { TickerProps, TickerItem } from "./components/Ticker";
export { HpBar } from "./components/HpBar";
export type { HpBarProps } from "./components/HpBar";
export { RoundRow } from "./components/RoundRow";
export type { RoundRowProps, RoundFighter } from "./components/RoundRow";
export { Bracket } from "./components/Bracket";
export type { BracketProps, BracketRound, BracketMatch, BracketEntry } from "./components/Bracket";
