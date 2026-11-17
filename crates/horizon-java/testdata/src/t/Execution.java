package t;

public class Execution {
    static int ticks = 7;
    interface Counter { int value(); }
    static class Base implements Counter { public int value() { return 3; } }
    static class Child extends Base { public int value() { return 9; } }
    static class OnlyArg { OnlyArg(int n) {} }
    public static int arithmetic() {
        int sum = 0;
        for (int i = 0; i < 10; i++) sum += i;
        return sum;
    }
    public static int dense(int n) {
        switch(n) { case 0: return 10; case 1: return 20; case 2: return 30; default: return -1; }
    }
    public static int sparse(int n) {
        switch(n) { case -100: return 4; case 1000: return 5; default: return -2; }
    }
    public static int arrays() { int[][] a = new int[2][3]; a[1][2] = 17; return a[1][2]; }
    public static boolean identity() { Object a = new Object(); Object b = a; return a == b && a != new Object(); }
    public static int dispatch() { Counter c = new Child(); return c.value(); }
    public static int lambda() { int n = 5; Runnable r = () -> ticks += n; r.run(); return ticks; }
    public static String concat() { return "tick=" + ticks; }
    public static String className() { return Execution.class.getName(); }
    public static int exception() { try { int z = 0; return 5 / z; } catch (ArithmeticException e) { return 42; } }
    public static int nullException() { try { Object a = null; return a.hashCode(); } catch (NullPointerException e) { return 43; } }
    public static long category2() { long[] a = {3}; return a[0]++; }
    public static int oversizedArray() { return new int[Integer.MAX_VALUE].length; }
    public static boolean staticIdentity() { return System.out == System.out; }
    public static void forever() { while (true) ticks++; }
    public static void unsupported() { System.gc(); }
}
