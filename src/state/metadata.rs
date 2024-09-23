struct MetadataMap<K, V> {
    map: HashMap<K, MetadataKey<V>>
}

struct MetadataKey<V> {
    value: V,

}
