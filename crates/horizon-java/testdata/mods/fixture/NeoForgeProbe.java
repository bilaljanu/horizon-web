package fixture;
import net.neoforged.fml.common.Mod;
@Mod("neoprobe")
public class NeoForgeProbe {
    public NeoForgeProbe() {
        Registrations.register("neoprobe");
        System.out.println("neoforge constructor executed");
    }
}
