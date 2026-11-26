<script lang="ts">
import { Card, Grid, Line, Preference, Split, Tag } from "@perishlab/design";
import type { Declaration, Item, View } from "../../../workbench.ts";

let {
	view,
	level,
	busy,
	ready = true,
	error,
	change,
	restore,
}: {
	view: View;
	level: string;
	busy: boolean;
	ready?: boolean;
	error?: string;
	change: (name: string, value: string) => void;
	restore: (name: string) => void;
} = $props();

const labels: Record<string, string> = {
	"theme.family": "视觉体系（theme.family）",
	"theme.tone": "明暗外观（theme.tone）",
	google: "Google（历史兼容）",
	"google-accounts": "Google Accounts",
	"google-workspace": "Google Workspace",
	material: "Material",
	system: "跟随设备",
	light: "浅色",
	dark: "深色",
};

const sources: Record<Item["source"], string> = {
	default: "默认设置",
	shared: "共享基线",
	app: "应用覆盖",
	policy: "策略约束",
};

function label(value: string): string {
	return labels[value] ?? value;
}

function describe(entry: Declaration): string {
	return entry.state === "present" ? label(entry.value) : "未声明（继承）";
}

function summary(item: Item): string {
	const shared = describe(item.shared);
	const app = describe(item.app);
	const tail =
		level === "shared"
			? `共享声明：${shared}`
			: `共享声明：${shared} · 应用声明：${app}`;
	return `当前生效：${label(item.effective)}（${sources[item.source]}） · ${tail}`;
}
</script>

<Grid cols={2} look="loose">
	{#each Object.values(view.items) as item (item.name)}
		<Card title={label(item.name)}>
			<Line name={item.name} meta={item.editable ? "可编辑" : "受策略锁定只读"} flow="fold">
				<Split look="close">
					<Tag text={sources[item.source]} look="quiet" />
					<Tag
						text={item.editable ? "允许修改" : "策略只读"}
						mood={item.editable ? "calm" : "warn"}
						look="quiet"
					/>
				</Split>
			</Line>
			<Preference
				name={`${level}:${item.name}`}
				label={label(item.name)}
				value={item.effective}
				source={item.source}
				declaration={level === "shared" ? item.shared : item.app}
				origin={sources[item.source]}
				effect={summary(item)}
				choices={item.options.map((value) => ({ value, label: label(value) }))}
				editable={item.editable && ready}
				{busy}
				{error}
				inherit={level === "shared" ? "恢复默认设置" : "继承共享或默认设置"}
				look="list"
				change={(value) => change(item.name, value)}
				restore={() => restore(item.name)}
			/>
		</Card>
	{/each}
</Grid>
