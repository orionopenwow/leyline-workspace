/*
 * mod-leyline-qol loader. AzerothCore calls Add<module dir name, '-' as '_'>Scripts() for each module in
 * modules/ (the generated ModulesLoader.cpp).
 */

void AddLeylineQolScripts();

void Addmod_leyline_qolScripts()
{
    AddLeylineQolScripts();
}
