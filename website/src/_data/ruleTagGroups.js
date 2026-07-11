import rules from "./rules.json" with { type: "json" };

const LABELS = {
  core: ["Cross-cutting core", "跨领域核心规则"],
  aliases: ["Type aliases", "类型别名"],
  annotations: ["Annotations", "类型注解"],
  callables: ["Callables", "可调用对象"],
  classes: ["Classes", "类"],
  constructors: ["Constructors", "构造器"],
  dataclasses: ["Dataclasses", "数据类"],
  directives: ["Typing directives", "类型指令"],
  enums: ["Enums", "枚举"],
  exceptions: ["Exceptions", "异常"],
  generics: ["Generics", "泛型"],
  historical: ["Historical behavior", "历史行为"],
  literals: ["Literals", "字面量"],
  namedtuples: ["Named tuples", "命名元组"],
  narrowing: ["Type narrowing", "类型缩小"],
  overloads: ["Overloads", "重载"],
  protocols: ["Protocols", "协议"],
  qualifiers: ["Type qualifiers", "类型限定符"],
  specialtypes: ["Special types", "特殊类型"],
  tuples: ["Tuples", "元组"],
  typeddicts: ["Typed dictionaries", "类型字典"],
  typeforms: ["Type forms", "类型形式"],
  strictness: ["Strictness", "严格性"],
  dependencies: ["Dependencies", "依赖管理"],
  style: ["Style", "代码风格"],
  imports: ["Imports", "导入"],
  redundancy: ["Redundancy", "冗余代码"],
  stubs: ["Type stubs", "类型存根"],
};

const PEP_ORDER = [
  "core", "aliases", "annotations", "callables", "classes", "constructors",
  "dataclasses", "directives", "enums", "exceptions", "generics", "historical",
  "literals", "namedtuples", "narrowing", "overloads", "protocols", "qualifiers",
  "specialtypes", "tuples", "typeddicts", "typeforms",
];

const BASILISK_ORDER = [
  "strictness", "dependencies", "style", "imports", "redundancy", "stubs",
];

function rulesFor(provenance, tag) {
  return rules.filter((rule) => {
    if (rule.provenance !== provenance) return false;
    if (tag === "core") return rule.tags.length === 1;
    return rule.tags.includes(tag);
  });
}

function makeGroups(provenance, order) {
  return order
    .map((tag) => {
      const items = rulesFor(provenance, tag);
      const [label, labelZh] = LABELS[tag];
      return {
        provenance,
        tag,
        id: tag,
        label,
        labelZh,
        count: items.length,
        items,
        url: `/docs/rules/${provenance}/${tag}/`,
        zhUrl: `/zh/docs/rules/${provenance}/${tag}/`,
      };
    })
    .filter((group) => group.count > 0);
}

const basilisk = makeGroups("basilisk", BASILISK_ORDER);
const pep = makeGroups("pep", PEP_ORDER);

export default {
  basilisk,
  pep,
  pages: [...basilisk, ...pep],
  counts: {
    basilisk: rules.filter((rule) => rule.provenance === "basilisk").length,
    pep: rules.filter((rule) => rule.provenance === "pep").length,
  },
};
