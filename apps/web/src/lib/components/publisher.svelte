<script lang="ts">
import { Button, Drawer, Field, Form, Note, Split } from "@perishlab/design";
import { held } from "../../../authorization.ts";
import { Failure } from "../../../client.ts";
import { create, parse, publish } from "../../../templates.ts";

let {
	mode,
	target,
	base,
	open = $bindable(false),
	done,
}: {
	mode: "create" | "edition";
	target: string;
	base: number;
	open?: boolean;
	done: (name: string, number: number) => void;
} = $props();

const sample = JSON.stringify(
	{
		$schema: "https://json-schema.org/draft/2020-12/schema",
		type: "object",
		properties: {
			title: { type: "string", minLength: 1 },
		},
		required: ["title"],
		additionalProperties: false,
	},
	null,
	2,
);

let name = $state("");
let body = $state(sample);
let busy = $state(false);
let warning = $state("");
let status = $state("");
let hold = $state(false);

$effect(() => {
	if (open) {
		warning = "";
		status = "";
		hold = false;
		if (mode === "create") name = "";
	}
});

function close(): void {
	open = false;
}

async function submit(): Promise<void> {
	busy = true;
	warning = "";
	status = "";
	hold = false;
	try {
		const schema = parse(body);
		const options = { access: held };
		const created =
			mode === "create"
				? await create({ name, schema }, options)
				: await publish({ name: target, base, schema }, options);
		open = false;
		done(created.template, created.number);
	} catch (error) {
		if (error instanceof Failure) {
			status = error.status > 0 ? String(error.status) : "NET";
			hold = error.uncertain;
			warning =
				error.status === 403
					? "当前身份未被授予模板管理权限（Keel 拒绝发布）。普通用户仍可只读浏览所有模板与模式。"
					: error.message;
		} else {
			status = "NET";
			hold = true;
			warning = "发布结果尚未确认，请先刷新模板列表确认当前状态。";
		}
	} finally {
		busy = false;
	}
}
</script>

<Drawer
	title={mode === "create" ? "发布新模板" : `发布新版本 · ${target}`}
	line={mode === "create"
		? "提交 Draft 2020-12 模式以注册模板第 1 版（需管理员权限）。"
		: `基于当前最新第 ${base} 版发布不可变第 ${base + 1} 版（需管理员权限）。`}
	bind:open
>
	{#if warning}
		<Note
			title={status === "403" ? "权限不足" : hold ? "发布结果待确认" : "发布未通过"}
			code={status || undefined}
			text={warning}
			mood={status === "403" ? "calm" : hold ? "hold" : "warn"}
			live
		/>
	{/if}

	<Form label="模板发布表单" {submit} {busy}>
		{#if mode === "create"}
			<Field
				label="模板标识"
				bind:value={name}
				hint="例如：workspace-note"
				help="小写字母、数字或连字符，且不能为 meta。"
				required
			/>
		{/if}
		<Field
			label="Draft 2020-12 JSON Schema"
			kind="area"
			bind:value={body}
			help="服务端将按 meta 第 1 版与 Draft 2020-12 编译器校验。"
			required
		/>
		<Split look="end">
			<Button label="取消" look="quiet" press={close} />
			<Button
				label={mode === "create" ? "注册模板" : `发布第 ${base + 1} 版`}
				submit
				{busy}
			/>
		</Split>
	</Form>
</Drawer>
