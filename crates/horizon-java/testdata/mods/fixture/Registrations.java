package fixture;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockBehaviour;

public final class Registrations {
    public static void register(String namespace) {
        Block block = Registry.register(BuiltInRegistries.BLOCK,
            ResourceLocation.fromNamespaceAndPath(namespace, "ruby_block"),
            new Block(BlockBehaviour.Properties.of().strength(2.0f)));
        Registry.register(BuiltInRegistries.ITEM,
            ResourceLocation.fromNamespaceAndPath(namespace, "ruby_block"),
            new BlockItem(block, new Item.Properties()));
        Registry.register(BuiltInRegistries.ITEM,
            ResourceLocation.fromNamespaceAndPath(namespace, "ruby"),
            new Item(new Item.Properties().stacksTo(16)));
    }
}
