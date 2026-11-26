<script lang="ts">
import {
	Board,
	Button,
	Hero,
	Note,
	Search,
	Split,
	Table,
} from "@perishlab/design";
import { onMount } from "svelte";
import { held, subscribe as watch } from "../../authorization.ts";
import { Failure } from "../../client.ts";
import { list, type Template } from "../../templates.ts";
import Chrome from "../lib/components/chrome.svelte";
import Inspector from "../lib/components/inspector.svelte";
import Publisher from "../lib/components/publisher.svelte";
import { announce } from "../lib/session.ts";

let items = $state<Template[]>([]);
let loading = $state(false);
let warning = $state("");
let query = $state("");
let selected = $state<number | undefined>();
let active = $state("");
let inspecting = $state(false);
let publishing = $state(false);
let mode = $state<"create" | "edition">("create");
let target = $state("");
let base = $state(1);

async function load(): Promise<void> {
	if (!held()) {
		items = [];
		return;
	}
	loading = true;
	warning = "";
	try {
		const result = await list({ access: held });
		items = result.templates;
	} catch (error) {
		warning =
			error instanceof Failure
				? error.message
				: "暂时无法读取模板列表，请重试。";
	} finally {
		loading = false;
	}
}

onMount(() => {
	const stop = watch(() => void load());
	void load();
	return stop;
});

const filtered = $derived.by(() => {
	const needle = query.trim().toLowerCase();
	if (!needle) return items;
	return items.filter((item) => item.name.toLowerCase().includes(needle));
});

const rows = $derived(
	filtered.map((item) => [
		item.name,
		`v${item.latest}`,
		String(item.editions.length),
		item.builtin ? "内建" : "扩展",
	]),
);

function pick(index: number): void {
	selected = index;
	const item = filtered[index];
	if (!item) return;
	active = item.name;
	inspecting = true;
}

function create(): void {
	mode = "create";
	target = "";
	base = 1;
	publishing = true;
}

function release(name: string, latest: number): void {
	inspecting = false;
	mode = "edition";
	target = name;
	base = latest;
	publishing = true;
}

function published(name: string, number: number): void {
	announce(`已发布模板 ${name} 第 ${number} 版。`);
	void load();
	active = name;
	inspecting = true;
}
</script>

<Chrome route="/templates" title="模板与模式" reload={() => void load()}>
	{#snippet children()}
		<Hero
			title="模板、版本与模式工作台"
			line="浏览内建与扩展模板的不可变版本、Draft 2020-12 结构化模式与快速兼容性状态，管理员可注册新模板或发布新版本。"
			look="plain"
		>
			{#snippet side()}
				<Button label="发布新模板" press={create} />
			{/snippet}
		</Hero>

		{#if warning}
			<Note text={warning} mood="warn" live>
				<Button label="重新读取" look="quiet" sign="sync" press={() => void load()} />
			</Note>
		{/if}

		<Board
			title="已注册模板目录"
			line={`共 ${items.length} 个模板 · 点击任一行查看版本历史、Draft 2020-12 Schema 与兼容性状态`}
		>
			{#snippet side()}
				<Split look="close">
					<Search bind:value={query} hint="按模板名称筛选" label="筛选模板" />
				</Split>
			{/snippet}
			<Table
				label="模板目录列表"
				heads={["模板名称", "最新版本", "版本数", "来源"]}
				{rows}
				{selected}
				{pick}
				state={loading && items.length === 0
					? "loading"
					: warning && items.length === 0
						? "error"
						: rows.length === 0
							? "empty"
							: "ready"}
				note={warning || "未找到匹配的模板。"}
				retry={() => void load()}
			/>
		</Board>

		<Inspector name={active} bind:open={inspecting} {release} />
		<Publisher
			{mode}
			{target}
			{base}
			bind:open={publishing}
			done={published}
		/>
	{/snippet}
</Chrome>
