<script lang="ts">
import {
	Button,
	Drawer,
	Field,
	Form,
	Note,
	Pick,
	Split,
} from "@perishlab/design";
import { held } from "../../../authorization.ts";
import { Failure } from "../../../client.ts";
import { create, type Entry, parse } from "../../../documents.ts";
import type { Template } from "../../../templates.ts";

let {
	templates,
	open = $bindable(false),
	created,
}: {
	templates: Template[];
	open?: boolean;
	created: (entry: Entry) => void;
} = $props();

const usable = $derived(templates.filter((item) => item.name !== "meta"));
const choices = $derived(
	usable.map((item) => ({
		value: item.name,
		label: `${item.name}（最新 v${item.latest}）`,
	})),
);

let id = $state("");
let template = $state("");
let body = $state("{}");
let busy = $state(false);
let warning = $state("");
let code = $state("");
let hold = $state(false);

const seeds: Record<string, unknown> = {
	tags: { tags: {} },
	theme: { "theme.family": "google-workspace", "theme.tone": "system" },
};

function seed(name: string): string {
	return JSON.stringify(seeds[name] ?? {}, null, 2);
}

$effect(() => {
	if (!open) return;
	warning = "";
	code = "";
	hold = false;
	if (!template && usable[0]) {
		template = usable[0].name;
		body = seed(usable[0].name);
	}
});

function pick(next: string): void {
	template = next;
	body = seed(next);
}

function close(): void {
	open = false;
}

async function submit(): Promise<void> {
	busy = true;
	warning = "";
	code = "";
	hold = false;
	try {
		const document = parse(body);
		const entry = await create({ id, template, document }, { access: held });
		open = false;
		id = "";
		created(entry);
	} catch (error) {
		if (error instanceof Failure) {
			code = error.status > 0 ? String(error.status) : "NET";
			hold = error.uncertain;
			warning = error.message;
		} else {
			code = "NET";
			hold = true;
			warning = "保存结果尚未确认，请先刷新文档列表确认是否已创建。";
		}
	} finally {
		busy = false;
	}
}
</script>

<Drawer
	title="新建结构化文档"
	line="选择非 meta 模板并提交符合对应版本 Draft 2020-12 模式的初始文档。"
	bind:open
>
	{#if warning}
		<Note
			title={hold ? "创建结果待确认" : "创建未通过"}
			code={code || undefined}
			text={warning}
			mood={hold ? "hold" : "warn"}
			live
		/>
	{/if}

	<Form label="新建文档表单" {submit} {busy}>
		<Field
			label="文档标识（slug）"
			bind:value={id}
			hint="例如：workspace-profile"
			help="仅支持小写字母、数字与连字符，不可使用保留名 preferences、workbench、tags。"
			required
		/>
		<Pick label="绑定模板" value={template} {choices} change={pick} />
		<Field
			label="初始 JSON 文档"
			kind="area"
			bind:value={body}
			help="提交时服务端将按照模板最新版本的 Draft 2020-12 模式校验。"
			required
		/>
		<Split look="end">
			<Button label="取消" look="quiet" press={close} />
			<Button label="创建文档" submit {busy} />
		</Split>
	</Form>
</Drawer>
