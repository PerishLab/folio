<script lang="ts">
import {
	Button,
	Card,
	Drawer,
	Line,
	Note,
	Pick,
	Schema,
	Split,
	Table,
	Tag,
} from "@perishlab/design";
import { held } from "../../../authorization.ts";
import { Failure } from "../../../client.ts";
import {
	type Brief,
	detail,
	type Edition,
	edition,
	type Status,
	status,
	type Template,
} from "../../../templates.ts";

let {
	name,
	open = $bindable(false),
	release,
}: {
	name: string;
	open?: boolean;
	release: (template: string, base: number) => void;
} = $props();

let loading = $state(false);
let warning = $state("");
let info = $state<Template<Brief> | undefined>();
let current = $state<Edition | undefined>();
let report = $state<Status | undefined>();
let chosen = $state("");
let seq = 0;

async function load(target: string, number?: number): Promise<void> {
	const ticket = ++seq;
	loading = true;
	warning = "";
	info = undefined;
	current = undefined;
	report = undefined;
	chosen = "";
	try {
		const options = { access: held };
		const found = await detail({ name: target }, options);
		if (ticket !== seq) return;
		info = found;
		const pick = number ?? found.latest;
		chosen = String(pick);
		const [loaded, checked] = await Promise.all([
			edition({ name: target, number: pick }, options),
			status({ name: target }, options),
		]);
		if (ticket !== seq) return;
		current = loaded;
		report = checked;
	} catch (error) {
		if (ticket !== seq) return;
		warning =
			error instanceof Failure
				? error.message
				: "暂时无法读取模板详情，请重试。";
	} finally {
		if (ticket === seq) loading = false;
	}
}

async function select(next: string): Promise<void> {
	chosen = next;
	const number = Number(next);
	if (!Number.isInteger(number) || number < 1) return;
	const ticket = ++seq;
	loading = true;
	warning = "";
	try {
		const loaded = await edition({ name, number }, { access: held });
		if (ticket !== seq) return;
		current = loaded;
	} catch (error) {
		if (ticket !== seq) return;
		warning =
			error instanceof Failure
				? error.message
				: "暂时无法读取指定版本模式。";
	} finally {
		if (ticket === seq) loading = false;
	}
}

$effect(() => {
	if (open && name) {
		void load(name);
	}
});

const choices = $derived(
	(info?.editions ?? []).map((item) => ({
		value: String(item.number),
		label: `第 ${item.number} 版（修订 ${item.revision.slice(0, 8)}）`,
	})),
);

const faults = $derived(
	(report?.incompatible ?? []).map((item) => [
		item.id,
		`v${item.edition}`,
		item.reason,
	]),
);
</script>

<Drawer title={`模板详情 · ${name}`} line={info?.builtin ? "系统内建模板" : "自定义发布模板"} bind:open>
	{#snippet side()}
		{#if info && name !== "meta"}
			<Button label="发布新版本" look="quiet" press={() => release(name, info?.latest ?? 1)} />
		{/if}
	{/snippet}

	{#if warning}
		<Note text={warning} mood="warn" live />
	{/if}

	{#if loading && !info}
		<Note text="正在读取模板版本与 Draft 2020-12 模式…" live />
	{:else if info}
		<Card title="版本选择">
			<Split look="apart">
				<Pick
					label="查看版本"
					value={chosen}
					{choices}
					change={(next) => void select(next)}
				/>
				<Split look="close">
					<Tag text={`最新 v${info.latest}`} />
					<Tag text={info.builtin ? "内建" : "扩展"} look="quiet" />
				</Split>
			</Split>
		</Card>

		{#if current}
			<Schema
				title={`${current.template} · Edition ${current.number}`}
				schema={current.schema}
				tree="结构树"
				raw="JSON"
			/>
		{/if}

		{#if report}
			<Card title="快速兼容性检查（GET /status）">
				<Line name="兼容状态" meta={`格式 ${report.format} · 检查目标 v${report.latest}`}>
					<Tag
						text={report.ready ? "全部兼容" : "存在不兼容文档"}
						mood={report.ready ? "calm" : "warn"}
					/>
				</Line>
				<Line
					name="检查统计"
					meta={`已检查 ${report.checked} 份文档，兼容 ${report.compatible} 份，不兼容 ${report.incompatible.length} 份`}
				/>
				{#if faults.length > 0}
					<Table
						label="不兼容文档列表"
						heads={["文档标识", "当前版本", "原因"]}
						rows={faults}
					/>
				{/if}
			</Card>
		{/if}
	{/if}
</Drawer>
