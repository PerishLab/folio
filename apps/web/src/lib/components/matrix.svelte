<script lang="ts">
import { Board, Card, Grid, Line, Split, Table, Tag } from "@perishlab/design";
import type { Board as Model, Presentation, Scopes } from "../../../workbench.ts";

let {
	board,
	scopes,
	revision,
	presentation,
}: {
	board: Model;
	scopes?: Scopes;
	revision: string;
	presentation: Presentation;
} = $props();

const origins: Record<string, string> = {
	default: "默认",
	shared: "共享",
	app: "覆盖",
	policy: "策略",
};

const bases: Record<string, string> = {
	setting: "明确配置",
	device: "设备偏好",
	default: "默认回退",
};

const heads = $derived([
	"作用域",
	"类型",
	...Object.keys(board.shared.items),
	"渲染体系",
	"渲染明暗",
]);

function cell(
	items: Model["shared"]["items"],
	key: string,
): string {
	const item = items[key];
	if (!item) return "—";
	return `${item.effective} (${origins[item.source] ?? item.source})`;
}

const rows = $derived.by(() => {
	const keys = Object.keys(board.shared.items);
	const list: string[][] = [
		[
			"shared",
			"共享基线",
			...keys.map((key) => cell(board.shared.items, key)),
			board.shared.presentation?.system ?? "—",
			board.shared.presentation?.tone ?? "—",
		],
	];
	for (const app of board.scopes) {
		const view = board.apps[app];
		if (!view) continue;
		const slot = scopes?.scopes.find((entry) => entry.name === app);
		list.push([
			app,
			slot && !slot.builtin ? "扩展应用" : "内建应用",
			...keys.map((key) => cell(view.items, key)),
			view.presentation?.system ?? "—",
			view.presentation?.tone ?? "—",
		]);
	}
	return list;
});
</script>

<Grid cols={2}>
	<Card title="Folio 实时渲染状态">
		<Line name="设计体系（system）" meta={presentation.system} flow="fold">
			<Tag text={presentation.system} />
		</Line>
		<Line name="明暗主题（tone）" meta={presentation.tone} flow="fold">
			<Tag text={presentation.tone === "dark" ? "深色" : "浅色"} />
		</Line>
		<Line name="明暗决策依据（basis）" meta={bases[presentation.basis] ?? presentation.basis} flow="fold">
			<Tag text={presentation.basis} look="quiet" />
		</Line>
		<Line name="文档修订版本" meta={revision ? revision.slice(0, 16) : "—"} flow="fold" />
	</Card>
	<Card title="已注册作用域">
		<Line name="作用域总数" meta={`共 ${board.scopes.length + 1} 个作用域`} flow="fold">
			<Split look="close">
				<Tag text="shared" />
				{#each board.scopes as app (app)}
					<Tag text={app} look="quiet" />
				{/each}
			</Split>
		</Line>
		<Line
			name="目录定义数"
			meta={`共 ${scopes?.definitions.length ?? Object.keys(board.shared.items).length} 项代码声明`}
			flow="fold"
		/>
	</Card>
</Grid>

<Board title="跨作用域生效总览" line="展示共享基线与所有已注册应用作用域的最终生效值与渲染结果。">
	<Table label="跨作用域设置对照表" {heads} {rows} />
</Board>
