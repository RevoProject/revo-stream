<script lang="ts">
  import LegacyFiltersModal from "../Modal/FiltersModal.svelte";

  type FilterItem = {
    id: string;
    name: string;
    kind: string;
    enabled: boolean;
    locked?: boolean;
    params: Record<string, string>;
  };

  export let open = false;
  export let targetType: "source" | "scene" = "source";
  export let targetId = "";
  export let targetLabel = "";
  export let filters: FilterItem[] = [];
</script>

<LegacyFiltersModal {open} {targetType} {targetId} {targetLabel} {filters} on:close on:save on:liveChange />
