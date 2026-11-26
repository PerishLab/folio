<script lang="ts">
import { Frame, Hero, Link, Note, Sheet, Shell } from "@perishlab/design";
import { onMount } from "svelte";
import { complete, configuration, Refusal } from "../../authorization.ts";
import { device } from "../lib/session.ts";
import { local } from "../lib/vault.ts";

let warning = $state("");
let tone = $state(device());

onMount(() => {
	tone = device();
	const url = location.href;
	history.replaceState(null, "", "/callback");
	void configuration()
		.then(async (config) => {
			if (!config) throw new Refusal("尚未配置身份提供方，请返回首页重试。");
			const back = await complete(config, {
				url,
				storage: sessionStorage,
				fallback: local(),
				fetcher: fetch,
			});
			history.replaceState(null, "", back);
			dispatchEvent(new PopStateEvent("popstate"));
		})
		.catch((error) => {
			warning =
				error instanceof Refusal
					? error.message
					: "暂时无法确认身份，请返回首页重试。";
		});
});
</script>

<svelte:head><title>Folio · 确认身份</title></svelte:head>

<Shell system="google-workspace" {tone}>
	<Frame look="center">
		<Sheet>
			<Hero title="确认身份" line="正在交换 Folio 资源访问凭据。" look="brief" />
			{#if warning}
				<Note text={warning} mood="warn" live />
				<Link href="/" label="返回 Folio" />
			{:else}
				<Note text="请稍候…" live />
			{/if}
		</Sheet>
	</Frame>
</Shell>
