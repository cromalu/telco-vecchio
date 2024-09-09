'use strict';

'require view';
'require ui';
'require form';
'require rpc';
'require fs';

return view.extend({

    render: function() {
        var mapdata = { actions: {}};
        var m = new form.JSONMap(mapdata, _('Telco-Vecchio Management'));
        var s = m.section(form.NamedSection, 'actions', _('Actions'));

	    //config
        o = s.option(form.SectionValue, 'actions', form.NamedSection, 'actions', 'actions', _('Configuration Management'), _('Check and update telco-vecchio configuration file'));
        var ss = o.subsection;
       var o = ss.option(form.Button, 'download', _('Download current configuration file'), _('In case you need to check the configuration'));
        o.inputtitle = _('Download');
       o.onclick = L.bind(function(ev) {
           var form = E('form', {
               'method': 'post',
               'action': L.env.cgi_base + '/cgi-download',
               'enctype': 'application/x-www-form-urlencoded'
           }, [
               E('input', { 'type': 'hidden', 'name': 'sessionid', 'value': rpc.getSessionID() }),
               E('input', { 'type': 'hidden', 'name': 'path',      'value': '/etc/telco-vecchio.conf' }),
               E('input', { 'type': 'hidden', 'name': 'filename',  'value': 'telco-vecchio.conf'})
           ]);
          ev.currentTarget.parentNode.appendChild(form);
          form.submit();
          form.parentNode.removeChild(form);
       }, this);
        var o = ss.option(form.Button, 'upload', _('Upload a new configuration file'), _('In case you need to change the configuration'));
        o.inputtitle = _('Upload');
        o.onclick = L.bind(function(ev) {
            return ui.uploadFile('/etc/telco-vecchio.conf',ev).then(function() {
                   ui.showModal(_('Success'), [
                       E('p', [ _('Configuration file uploaded successfuly') ]),
                       E('div', { 'class': 'right' }, [
                           E('button', { 'click': ui.hideModal }, [ _('OK') ])
                       ])]);
            }).catch(function(err) {
                ui.addNotification(null, E('p', [ _('Configuration file upload failed: %s').format(err.message) ]));
            });
        }, this);


        //Restart service
        o = s.option(form.SectionValue, 'actions', form.NamedSection, 'actions', 'actions', _('Service Management'), _('Restart service'));
        var ss = o.subsection;
        var o = ss.option(form.Button, 'restart', _('Restart Telco-Vecchio service'), _('In case new binary or configuration must be applied'));
        o.inputtitle = _('Restart');
        o.onclick = L.bind(function(ev) {
            L.ui.showModal(_('Restarting…'), [
                E('p', { 'class': 'spinning' }, _('Service is restarting'))
            ]);
            fs.exec('/etc/init.d/telco-vecchio', ['restart']).then(L.bind(function() {
                ui.showModal(_('Success'), [
                    E('p', [ _('Service restarted') ]),
                    E('div', { 'class': 'right' }, [
                        E('button', { 'click': ui.hideModal }, [ _('OK') ])
                    ])]);
            }, this)).catch(function(err) {
                ui.addNotification(null, E('p', [ _('Service restart failed: %s').format(err.message) ]));
            });
        }, this);

        //logs
        o = s.option(form.SectionValue, 'actions', form.NamedSection, 'actions', 'actions', _('Log Download'), _('Download Telco-Vecchio log files'));
        var ss = o.subsection;
        var o = ss.option(form.Button, 'dl_logs', _(''), _(''));
        o.inputtitle = _('Download');
        o.onclick = L.bind(function(ev) {
            var form = E('form', {
                'method': 'post',
                'action': L.env.cgi_base + '/cgi-download',
                'enctype': 'application/x-www-form-urlencoded'
            }, [
                E('input', { 'type': 'hidden', 'name': 'sessionid', 'value': rpc.getSessionID() }),
                E('input', { 'type': 'hidden', 'name': 'path',      'value': '/tmp/log/telco-vecchio/log' }),
                E('input', { 'type': 'hidden', 'name': 'filename',  'value': 'log'})
            ]);
            ev.currentTarget.parentNode.appendChild(form);
            form.submit();
            form.parentNode.removeChild(form);
        }, this);

        return m.render();
    }
});

